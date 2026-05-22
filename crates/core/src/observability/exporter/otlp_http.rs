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
//! 当前为 stub，仅满足 CompositeSpanExporter 编译；Task 2.3 会替换为真实实现。

use futures::future::BoxFuture;
use opentelemetry_sdk::export::trace::{ExportResult, SpanData, SpanExporter};

/// OTLP HTTP 上报导出器（stub）。
#[derive(Debug)]
pub struct OtlpHttpExporter;

impl OtlpHttpExporter {
    /// 占位构造函数：真实实现见 Task 2.3。
    pub fn new(_endpoint: &str, _public_key: &str, _secret_key: &str) -> anyhow::Result<Self> {
        Ok(Self)
    }
}

impl SpanExporter for OtlpHttpExporter {
    fn export(&mut self, _batch: Vec<SpanData>) -> BoxFuture<'static, ExportResult> {
        Box::pin(async { Ok(()) })
    }
}
