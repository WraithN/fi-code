// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! TracerProvider 装配模块。
//!
//! 职责：
//! - 解析日志目录（`~/.config/fi-code/logs/`），唯一会冒泡的失败路径。
//! - 构造 `LocalJsonlExporter`（必成功）+ 可选 `OtlpHttpExporter`，组装为 `CompositeSpanExporter`。
//! - 用 `BatchSpanProcessor` 串入 `TracerProvider`，注入服务级 `Resource` 属性。
//! - 提供 `local_exporter()` 给重发 daemon 访问同一 LocalJsonl 句柄。
//!
//! 设计要点：
//! - 仅日志目录创建失败时返回 Err；OTLP 构造失败只 log_warn 后降级为单写本地。
//! - `PROVIDER` / `LOCAL_EXPORTER` 用 `OnceLock` 持有，保证全局唯一且线程安全。

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use opentelemetry::{global, KeyValue};
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, TracerProvider};
use opentelemetry_sdk::Resource;

use crate::log_warn;
use crate::observability::attrs::{LANGFUSE_ENVIRONMENT, LANGFUSE_RELEASE};
use crate::observability::config::ObservabilityConfig;
use crate::observability::exporter::local_jsonl::LocalJsonlExporter;
use crate::observability::exporter::otlp_http::OtlpHttpExporter;
use crate::observability::exporter::CompositeSpanExporter;

// ===== 模块级常量（AGENTS.md §6.11 禁止魔法值）=====

/// OTel `service.name` 资源属性。
const SERVICE_NAME: &str = "fi-code";
/// OTel `service.name` 资源属性键。
const SERVICE_NAME_KEY: &str = "service.name";
/// OTel `service.version` 资源属性键。
const SERVICE_VERSION_KEY: &str = "service.version";
/// OTel `deployment.environment` 资源属性键。
const DEPLOYMENT_ENVIRONMENT_KEY: &str = "deployment.environment";
/// 当前 crate 版本，写入 `service.version`。
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 默认部署环境（langfuse.environment 缺省时使用）。
const DEFAULT_ENVIRONMENT: &str = "dev";
/// fi-code 配置目录下放日志的子目录名。
const LOGS_DIR_NAME: &str = "logs";
/// span 落盘的 JSONL 文件名。
const SPANS_FILE_NAME: &str = "spans.jsonl";
/// 旧版 turn 日志文件名；若存在则打印告警提醒用户清理。
const LEGACY_TURNS_FILE_NAME: &str = "turns.jsonl";
/// `directories::ProjectDirs` 三段式参数：qualifier / organization / application。
const PROJECT_QUALIFIER: &str = "";
const PROJECT_ORG: &str = "";
const PROJECT_APP: &str = "fi-code";

// ===== Batch 处理器参数（spec §3.2）=====
const BATCH_SIZE: usize = 512;
const QUEUE_SIZE: usize = 2048;
const SCHEDULED_DELAY_MS: u64 = 5000;

// ===== 全局单例：跨模块共享 LocalJsonlExporter 与 TracerProvider =====
/// LocalJsonl 句柄；重发 daemon 通过 `local_exporter()` 拿同一份。
static LOCAL_EXPORTER: OnceLock<Arc<LocalJsonlExporter>> = OnceLock::new();
/// TracerProvider 全局句柄；`shutdown()` 时调用其 `.shutdown()`。
static PROVIDER: OnceLock<TracerProvider> = OnceLock::new();

/// 解析并创建日志目录。失败时冒泡 Err（init 的唯一硬错误）。
fn logs_dir() -> anyhow::Result<PathBuf> {
    let proj = directories::ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORG, PROJECT_APP)
        .ok_or_else(|| {
            anyhow::anyhow!("无法解析 fi-code 配置目录（ProjectDirs::from 返回 None）")
        })?;
    let dir = proj.config_dir().join(LOGS_DIR_NAME);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 安装 TracerProvider。
///
/// - 必做：解析日志目录 + 构造 LocalJsonlExporter。失败则 Err 冒泡。
/// - 可选：langfuse_enabled() 时尝试构造 OtlpHttpExporter；失败仅 log_warn。
/// - 装配 CompositeSpanExporter → BatchSpanProcessor → TracerProvider。
/// - 将 provider 设为全局并存入 `PROVIDER`。
pub fn install(cfg: &ObservabilityConfig) -> anyhow::Result<()> {
    // 0. 幂等性保护：若 PROVIDER 已设置，直接返回 Ok，避免重复创建导致句柄泄漏与 provider 不可达。
    if PROVIDER.get().is_some() {
        crate::log_warn!("[observability] install() 被重复调用，已跳过");
        return Ok(());
    }

    // 1. 日志目录（硬错误）。
    let dir = logs_dir()?;

    // 2. 旧版 turns.jsonl 残留告警（不影响初始化）。
    let legacy = dir.join(LEGACY_TURNS_FILE_NAME);
    if legacy.exists() {
        log_warn!(
            "[observability] 检测到旧版 {} 文件，已不再使用，可手动清理：{:?}",
            LEGACY_TURNS_FILE_NAME,
            legacy
        );
    }

    // 3. LocalJsonlExporter（必成功；失败 → Err 冒泡）。
    let local_path = dir.join(SPANS_FILE_NAME);
    let local = Arc::new(LocalJsonlExporter::new(local_path)?);
    // 存全局供 resend daemon 访问；忽略 set 失败（重复 install 视为幂等）。
    let _ = LOCAL_EXPORTER.set(Arc::clone(&local));

    // 4. 可选 OtlpHttpExporter（失败仅 log_warn）。
    let otlp = if cfg.langfuse_enabled() {
        build_otlp(cfg)
    } else {
        None
    };

    // 5. 组合 + BatchProcessor。
    let composite = CompositeSpanExporter::new(Arc::clone(&local), otlp);
    let batch_cfg = BatchConfigBuilder::default()
        .with_max_export_batch_size(BATCH_SIZE)
        .with_max_queue_size(QUEUE_SIZE)
        .with_scheduled_delay(Duration::from_millis(SCHEDULED_DELAY_MS))
        .build();
    let processor = BatchSpanProcessor::builder(composite, opentelemetry_sdk::runtime::Tokio)
        .with_batch_config(batch_cfg)
        .build();

    // 6. Resource：service.name / service.version / deployment.environment / langfuse.release。
    let environment = cfg
        .langfuse
        .environment
        .clone()
        .unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string());
    let release = cfg
        .langfuse
        .release
        .clone()
        .unwrap_or_else(|| SERVICE_VERSION.to_string());
    let resource = Resource::new(vec![
        KeyValue::new(SERVICE_NAME_KEY, SERVICE_NAME),
        KeyValue::new(SERVICE_VERSION_KEY, SERVICE_VERSION),
        KeyValue::new(DEPLOYMENT_ENVIRONMENT_KEY, environment.clone()),
        KeyValue::new(LANGFUSE_RELEASE, release),
        // langfuse.environment 同步写入 Resource，便于 Langfuse UI 过滤；复用上方 environment 局部变量避免重复计算。
        KeyValue::new(LANGFUSE_ENVIRONMENT, environment),
    ]);

    // 7. 装配 provider 并注册到全局。
    let provider = TracerProvider::builder()
        .with_span_processor(processor)
        .with_resource(resource)
        .build();
    // global::set_tracer_provider 返回旧 provider；这里忽略即可。
    let _ = global::set_tracer_provider(provider.clone());
    let _ = PROVIDER.set(provider);
    Ok(())
}

/// 关闭 TracerProvider（flush 残留批次后停止 Tokio task）。
pub fn shutdown() {
    if let Some(p) = PROVIDER.get() {
        let _ = p.shutdown();
    }
}

/// 暴露给 resend 模块：拿到与导出器共享的 LocalJsonl 句柄。
pub(crate) fn local_exporter() -> Option<Arc<LocalJsonlExporter>> {
    LOCAL_EXPORTER.get().cloned()
}

/// 构造 OTLP exporter；构造失败仅记日志返回 None，不冒泡。
fn build_otlp(cfg: &ObservabilityConfig) -> Option<OtlpHttpExporter> {
    // 两把 key 必须同时存在，否则 langfuse_enabled 已置 false，这里只是 belt-and-suspenders。
    let (pk, sk) = match (
        cfg.langfuse.public_key.as_deref(),
        cfg.langfuse.secret_key.as_deref(),
    ) {
        (Some(p), Some(s)) => (p, s),
        _ => {
            log_warn!("[observability] Langfuse 启用但缺少 public/secret key，跳过 OTLP");
            return None;
        }
    };
    match OtlpHttpExporter::new(&cfg.langfuse.host, pk, sk) {
        Ok(e) => Some(e),
        Err(err) => {
            log_warn!(
                "[observability] OtlpHttpExporter 构造失败，仅写本地：{}",
                err
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证日志目录解析成功且路径以 logs 结尾。
    #[test]
    fn test_logs_dir_resolves() {
        let dir = logs_dir().expect("logs_dir 应当 Ok");
        assert!(
            dir.ends_with(LOGS_DIR_NAME),
            "logs_dir 末段应为 {}，实际：{:?}",
            LOGS_DIR_NAME,
            dir
        );
    }

    /// 禁用 Langfuse 的 install 必须返回 Ok 且不 panic。
    /// 该测试依赖运行环境对日志目录可写；CI/沙箱不可写时跳过即可。
    ///
    /// 注意事项：
    /// - 必须 multi_thread runtime：BatchSpanProcessor::builder 内部 `runtime.spawn`
    ///   需要 Tokio 反应器；多线程版本同时允许后续 `block_on` 不死锁。
    /// - 不主动调用 `shutdown()`：`BatchSpanProcessor::shutdown` 内部用
    ///   `futures_executor::block_on` 等待 oneshot，单线程或子线程上下文下可能死锁。
    ///   真正的关闭由进程退出时调用 `mod::shutdown()` 触发。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_install_with_disabled_langfuse_succeeds() {
        let cfg = ObservabilityConfig::default();
        if logs_dir().is_err() {
            eprintln!("跳过 install 测试：logs_dir 不可用");
            return;
        }
        let res = install(&cfg);
        assert!(res.is_ok(), "禁用态 install 必须 Ok: {:?}", res.err());
    }

    /// install() 必须幂等：重复调用不 panic、不重复创建 provider。
    /// 注意：与上面的测试共享同一全局 PROVIDER，但 OnceLock 的语义保证了第二次 install 直接跳过。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_install_is_idempotent() {
        let cfg = ObservabilityConfig::default();
        if logs_dir().is_err() {
            eprintln!("跳过 install 幂等性测试：logs_dir 不可用");
            return;
        }
        let r1 = install(&cfg);
        let r2 = install(&cfg);
        assert!(r1.is_ok(), "第一次 install 应 Ok：{:?}", r1.err());
        assert!(r2.is_ok(), "重复 install 应 Ok（幂等）：{:?}", r2.err());
    }
}
