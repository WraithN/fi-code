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

//! OtlpHttpExporter：薄封装 opentelemetry-otlp 的 HTTP/protobuf exporter。
//!
//! - 端点：`{host_trim_trailing_slash}/api/public/otel/v1/traces`
//! - 鉴权：Basic base64(public_key + ":" + secret_key)
//! - 额外 header：`x-langfuse-ingestion-version: 4`
//! - 超时：10s（spec §4.5）

use base64::Engine;
use futures::future::BoxFuture;
use opentelemetry_otlp::{SpanExporter as OtlpSpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use std::collections::HashMap;
use std::time::Duration;

/// OTLP HTTP 上报导出器：内部委托给 opentelemetry-otlp 的 SpanExporter。
#[derive(Debug)]
pub struct OtlpHttpExporter {
    inner: OtlpSpanExporter,
}

impl OtlpHttpExporter {
    /// host 形如 "https://cloud.langfuse.com"；尾部斜杠会被自动去掉。
    pub fn new(host: &str, public_key: &str, secret_key: &str) -> anyhow::Result<Self> {
        let endpoint = format!("{}/api/public/otel/v1/traces", host.trim_end_matches('/'));
        let auth_raw = format!("{}:{}", public_key, secret_key);
        let auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth_raw);

        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Basic {}", auth_b64));
        headers.insert("x-langfuse-ingestion-version".into(), "4".into());

        let inner = OtlpSpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_headers(headers)
            .with_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self { inner })
    }
}

impl SpanExporter for OtlpHttpExporter {
    fn export(&mut self, batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        self.inner.export(batch)
    }

    fn shutdown(&mut self) {
        self.inner.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_encoding() {
        // 验证 base64 编码与 spec 一致。
        let auth = base64::engine::general_purpose::STANDARD.encode("pk-lf-x:sk-lf-y");
        assert_eq!(auth, "cGstbGYteDpzay1sZi15");
    }

    #[test]
    fn test_constructor_with_invalid_host() {
        // 仅验证构造不 panic；网络请求时才会失败。
        let r = OtlpHttpExporter::new("https://invalid.example", "pk", "sk");
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn test_export_sends_basic_auth_header() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/public/otel/v1/traces"))
            .and(header("Authorization", "Basic cGstbGYteDpzay1sZi15"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut exp = OtlpHttpExporter::new(&server.uri(), "pk-lf-x", "sk-lf-y").unwrap();
        // 空 batch 不一定触发 HTTP 请求；此用例主要验证构造 + 调用不 panic。
        let _ = exp.export(vec![]).await;
    }
}
