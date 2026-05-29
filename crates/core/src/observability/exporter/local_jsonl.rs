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

//! LocalJsonlExporter：把 OTel SpanData 序列化为 JSONL 行写入 spans.jsonl。
//!
//! 关键点：
//! - append-only，单进程内用 Mutex<File> 保证不交错。
//! - 每行尾包 `lf_status="pending"`。
//! - 提供 append_status_patch() 由 CompositeExporter 在 OTLP 成功后调用。
//! - 写失败时返回 Err(TraceError)，由 OTel SDK 决定是否重试。

use opentelemetry::trace::TraceError;
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::log_error;

/// Langfuse 上报状态：尚未上报。
pub(crate) const LF_STATUS_PENDING: &str = "pending";
/// Langfuse 上报状态：已成功上报。
pub(crate) const LF_STATUS_SENT: &str = "sent";
/// JSONL 行的 type 字段值：status_patch（区别于完整 span 行）。
const PATCH_TYPE_STATUS: &str = "status";

/// 本地 JSONL 落盘导出器：每个 span 写一行 JSON，lf_status 永远为 "pending"。
#[derive(Debug)]
pub struct LocalJsonlExporter {
    file: Mutex<File>,
    path: PathBuf,
}

impl LocalJsonlExporter {
    /// 创建导出器，自动创建父目录，以 create + append 模式打开文件。
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    /// 返回底层文件路径。
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 追加 status_patch 行，标记一组 span_id 的 Langfuse 上报状态。
    pub fn append_status_patch(&self, span_ids: &[String], status: &str) {
        let patch = json!({
            "type": PATCH_TYPE_STATUS,
            "span_ids": span_ids,
            "lf_status": status,
            "patched_at_unix_nano": now_unix_nano(),
        });
        let line = serde_json::to_string(&patch).unwrap_or_default();
        if let Err(e) = self.write_line(&line) {
            log_error!("[observability] failed to write status patch: {}", e);
        }
    }

    /// 通过共享 `Arc<Self>` 也可调用的批量导出入口。
    ///
    /// 关键设计：与 `SpanExporter::export(&mut self, ...)` 不同，本方法只需要 `&self`，
    /// 因为底层文件句柄通过 `Mutex<File>` 自带内部可变性。
    /// CompositeSpanExporter 持有 `Arc<LocalJsonlExporter>`，无法借出 `&mut`，
    /// 故组合导出器走这条路径，避免再开第二个文件句柄导致写入交错。
    pub fn export_batch(&self, batch: Vec<SpanData>) -> ExportResult {
        for span in &batch {
            let line = span_to_jsonl(span);
            self.write_line(&line)
                .map_err(|e| TraceError::from(format!("local jsonl write: {}", e)))?;
        }
        Ok(())
    }

    /// 写入一行（加换行符），通过 Mutex 保证互斥。
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        let mut f = self
            .file
            .lock()
            .expect("LocalJsonlExporter file mutex poisoned");
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
        Ok(())
    }
}

impl SpanExporter for LocalJsonlExporter {
    fn export(
        &mut self,
        batch: Vec<SpanData>,
    ) -> futures::future::BoxFuture<'static, ExportResult> {
        // 直接复用 &self 版本：trait 要求 &mut，但内部互斥靠 Mutex<File>。
        let result = self.export_batch(batch);
        Box::pin(async move { result })
    }
}

/// 将 SpanData 序列化为单行 JSON（spec §3.4 格式）。
fn span_to_jsonl(span: &SpanData) -> String {
    let mut attrs = serde_json::Map::new();
    for kv in &span.attributes {
        attrs.insert(kv.key.to_string(), Value::String(kv.value.to_string()));
    }
    let obj = json!({
        "trace_id": span.span_context.trace_id().to_string(),
        "span_id": span.span_context.span_id().to_string(),
        "parent_span_id": span.parent_span_id.to_string(),
        "name": span.name,
        "kind": format!("{:?}", span.span_kind),
        "start_time_unix_nano": time_to_nanos(span.start_time),
        "end_time_unix_nano": time_to_nanos(span.end_time),
        "status": {
            "code": format!("{:?}", span.status),
        },
        "attributes": Value::Object(attrs),
        "events": [],
        "lf_status": LF_STATUS_PENDING,
    });
    serde_json::to_string(&obj).unwrap_or_default()
}

/// SystemTime → Unix 纳秒。
fn time_to_nanos(t: std::time::SystemTime) -> u128 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// 当前时间的 Unix 纳秒。
fn now_unix_nano() -> u128 {
    time_to_nanos(std::time::SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{
        SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
    };
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::trace::SpanLinks;
    use std::borrow::Cow;
    use std::time::SystemTime;
    use tempfile::tempdir;

    /// 构造一个用于测试的 SpanData。
    fn dummy_span(name: &str, trace_id_hex: &str, span_id_hex: &str) -> SpanData {
        SpanData {
            span_context: SpanContext::new(
                TraceId::from_hex(trace_id_hex).unwrap(),
                SpanId::from_hex(span_id_hex).unwrap(),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::INVALID,
            span_kind: SpanKind::Internal,
            name: Cow::Owned(name.to_string()),
            start_time: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            end_time: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
            attributes: vec![KeyValue::new("foo", "bar")],
            dropped_attributes_count: 0,
            events: opentelemetry_sdk::trace::SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Ok,
            instrumentation_scope: opentelemetry::InstrumentationScope::builder("test").build(),
        }
    }

    #[tokio::test]
    async fn test_export_writes_jsonl_with_pending_status() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("spans.jsonl");
        let mut exp = LocalJsonlExporter::new(path.clone()).unwrap();

        let span = dummy_span(
            "test.span",
            "0123456789abcdef0123456789abcdef",
            "0123456789abcdef",
        );
        exp.export(vec![span]).await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let line = content.lines().next().unwrap();
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(v["name"], "test.span");
        assert_eq!(v["lf_status"], "pending");
        assert_eq!(v["attributes"]["foo"], "bar");
    }

    #[tokio::test]
    async fn test_append_status_patch_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("spans.jsonl");
        let exp = LocalJsonlExporter::new(path.clone()).unwrap();
        exp.append_status_patch(&["a".into(), "b".into()], "sent");

        let content = std::fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(v["type"], "status");
        assert_eq!(v["lf_status"], "sent");
        assert_eq!(v["span_ids"], json!(["a", "b"]));
    }
}
