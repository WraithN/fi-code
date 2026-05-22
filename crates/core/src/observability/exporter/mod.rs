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

//! exporter 子模块：CompositeSpanExporter + LocalJsonlExporter + OtlpHttpExporter。
//!
//! 行为：
//! - LocalJsonl 必成功；写失败仅自身 log_error，不影响 OTLP。
//! - OTLP 失败时 log_warn，不冒泡，由启动期 daemon 补。
//! - OTLP 成功时调 local.append_status_patch(span_ids, "sent")。
//! - export() 始终返回 Ok（避免 BatchSpanProcessor 因 OTLP 失败丢整批 batch）。

pub mod local_jsonl;
pub mod otlp_http;

use futures::future::BoxFuture;
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use std::sync::Arc;

use crate::log_warn;

use local_jsonl::LocalJsonlExporter;
use otlp_http::OtlpHttpExporter;

/// 组合导出器：fan-out 到 LocalJsonl + 可选 OTLP。
#[derive(Debug)]
pub struct CompositeSpanExporter {
    pub(crate) local: Arc<LocalJsonlExporter>,
    pub(crate) otlp: Option<OtlpHttpExporter>,
}

impl CompositeSpanExporter {
    /// 构造组合导出器。
    pub fn new(local: Arc<LocalJsonlExporter>, otlp: Option<OtlpHttpExporter>) -> Self {
        Self { local, otlp }
    }
}

impl SpanExporter for CompositeSpanExporter {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        // 在消耗 batch 之前先收集 span_id，后续用于 status_patch。
        let local = Arc::clone(&self.local);
        let span_ids: Vec<String> = batch
            .iter()
            .map(|s| s.span_context.span_id().to_string())
            .collect();

        // 第一步：LocalJsonl 同步写（通过 bridge 包一层独立 handle 即可调用 export）。
        let mut local_exp = LocalJsonlBridge(Arc::clone(&local));
        let local_fut = local_exp.export(batch.clone());

        // 第二步：可选 OTLP（注意此时已 clone batch 给 local，OTLP 拿原 batch）。
        let otlp_fut = self.otlp.as_mut().map(|o| o.export(batch));

        Box::pin(async move {
            // local 失败已在内部 log_error，这里只 await 不冒泡。
            let _ = local_fut.await;
            if let Some(fut) = otlp_fut {
                match fut.await {
                    Ok(_) => local.append_status_patch(&span_ids, "sent"),
                    Err(_) => {
                        // OTLP 失败不冒泡，等启动期 daemon 补；
                        // log_warn! 宏含 #[cfg(debug_assertions)] 块表达式，
                        // 必须在语句位置调用而非表达式位置。
                        log_warn!("[observability] OTLP export failed");
                    }
                }
            }
            Ok(())
        })
    }
}

/// 把 Arc<LocalJsonlExporter> 包成可调用 SpanExporter::export 的临时桥。
/// 由于 LocalJsonlExporter::export 要求 &mut self，而我们持有的是 Arc<...>，
/// 故通过 clone_handle 重新打开同一文件路径，得到独立的可变 handle。
/// 写入仍走各自 Mutex<File>，O_APPEND 模式下不会交错。
#[derive(Debug)]
struct LocalJsonlBridge(Arc<LocalJsonlExporter>);

impl SpanExporter for LocalJsonlBridge {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        let arc = Arc::clone(&self.0);
        Box::pin(async move {
            let mut exp = LocalJsonlExporter::clone_handle(&arc);
            exp.export(batch).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{
        SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
    };
    use opentelemetry_sdk::export::trace::SpanData;
    use opentelemetry_sdk::trace::SpanLinks;
    use std::borrow::Cow;
    use std::time::SystemTime;
    use tempfile::tempdir;

    /// 构造一个空属性的最简 SpanData。
    fn dummy_span() -> SpanData {
        SpanData {
            span_context: SpanContext::new(
                TraceId::from_hex("0123456789abcdef0123456789abcdef").unwrap(),
                SpanId::from_hex("0123456789abcdef").unwrap(),
                TraceFlags::default(),
                false,
                TraceState::default(),
            ),
            parent_span_id: SpanId::INVALID,
            span_kind: SpanKind::Internal,
            name: Cow::Borrowed("t"),
            start_time: SystemTime::UNIX_EPOCH,
            end_time: SystemTime::UNIX_EPOCH,
            attributes: vec![],
            dropped_attributes_count: 0,
            events: opentelemetry_sdk::trace::SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Ok,
            instrumentation_scope: opentelemetry::InstrumentationScope::builder("test").build(),
        }
    }

    #[tokio::test]
    async fn test_composite_without_otlp_writes_local_only() {
        let dir = tempdir().unwrap();
        let local = Arc::new(LocalJsonlExporter::new(dir.path().join("spans.jsonl")).unwrap());
        let mut composite = CompositeSpanExporter::new(Arc::clone(&local), None);
        composite.export(vec![dummy_span()]).await.unwrap();
        let content = std::fs::read_to_string(dir.path().join("spans.jsonl")).unwrap();
        assert!(content.contains("\"lf_status\":\"pending\""));
        assert!(!content.contains("\"type\":\"status\""));
    }
}
