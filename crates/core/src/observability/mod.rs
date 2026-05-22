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

// 可观测性模块：负责 OpenTelemetry trace 采集与 Langfuse 上报
// 子模块说明：
// - attrs:    OTel / Langfuse / fi-code 自定义属性键常量
// - cli_view: CLI 侧的 trace 展示视图
// - config:   从 env 与 config.json 解析 ObservabilityConfig
// - exporter: 本地 JSONL 落盘 + OTLP HTTP 远程上报
// - facade:   对外暴露的统一门面，业务层只需调用此处的 API
// - redact:   凭据脱敏与超大 payload 截断
// - resend:   失败 trace 的重发机制
// - tracer:   tracer provider 初始化、span 构建辅助
pub mod attrs;
pub mod cli_view;
pub mod config;
pub mod exporter;
pub mod facade;
pub mod redact;
pub mod resend;
pub mod tracer;

// 别名导出：业务层使用 `observability::otel` 调用门面 API
pub use facade as otel;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::log_warn;
use crate::observability::config::ObservabilityConfig;

// 全局开关：是否启用可观测性。初始化成功后置为 true
static ENABLED: AtomicBool = AtomicBool::new(false);

/// 初始化可观测性子系统。
///
/// 行为：
/// - 解析 ObservabilityConfig（合并 env + config.json）。
/// - 调用 `tracer::install`：成功则置 ENABLED=true；
///   只有"日志目录不可创建"这一种情况会冒泡 Err，其余降级（如 OTLP 失败仅 log_warn）。
pub fn init(config: &crate::config::Config) -> anyhow::Result<()> {
    let obs_cfg = ObservabilityConfig::resolve(config);
    match tracer::install(&obs_cfg) {
        Ok(()) => {
            ENABLED.store(true, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => {
            log_warn!("[observability] tracer::install 失败：{}", e);
            Err(e)
        }
    }
}

/// 关闭可观测性子系统：转发到 tracer::shutdown 以 flush 残留 span。
pub fn shutdown() {
    tracer::shutdown();
}

/// 查询当前是否启用可观测性
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}
