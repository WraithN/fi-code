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

use axum::{
    extract::{Query, State},
    response::{sse::Event, Sse},
    Json,
};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::server::models::ApiResponse;
use crate::server::server::AppState;
use crate::utils::log_store::LogEntry;

#[derive(Deserialize)]
pub struct ListLogsQuery {
    limit: Option<usize>,
}

/// 列出最近的日志条目
pub async fn handle_list_logs(
    State(state): State<AppState>,
    Query(query): Query<ListLogsQuery>,
) -> Json<ApiResponse<Vec<LogEntry>>> {
    let limit = query.limit.unwrap_or(200).min(1000);
    let logs = match &state.log_broadcaster {
        Some(b) => b.recent(limit),
        None => Vec::new(),
    };
    Json(ApiResponse::success(logs))
}

/// SSE 流式推送日志
pub async fn handle_log_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = match &state.log_broadcaster {
        Some(b) => b.subscribe(),
        None => {
            let (tx, rx) = tokio::sync::broadcast::channel(1);
            drop(tx);
            rx
        }
    };

    let stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(entry) => {
                let data = serde_json::to_string(&entry).unwrap_or_default();
                Some(Ok::<_, Infallible>(Event::default().data(data)))
            }
            Err(_) => None,
        });

    Sse::new(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::server::test_helpers::create_test_app_state;
    use axum::{extract::State, response::IntoResponse};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_handle_list_logs_empty() {
        let state = create_test_app_state();
        let query = ListLogsQuery { limit: None };
        let response = handle_list_logs(State(state), Query(query)).await;

        assert!(response.0.success);
        let logs = response.0.data.unwrap();
        assert!(logs.is_empty());
    }

    #[tokio::test]
    async fn test_handle_list_logs_with_entries() {
        let state = create_test_app_state();

        // 先发送一些日志
        if let Some(broadcaster) = &state.log_broadcaster {
            broadcaster.send("INFO", "test_module", "test message 1".to_string());
            broadcaster.send("DEBUG", "test_module", "test message 2".to_string());
            broadcaster.send("ERROR", "test_module", "test message 3".to_string());
        }

        let query = ListLogsQuery { limit: Some(10) };
        let response = handle_list_logs(State(state), Query(query)).await;

        assert!(response.0.success);
        let logs = response.0.data.unwrap();
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "test message 1");
        assert_eq!(logs[1].level, "DEBUG");
        assert_eq!(logs[2].level, "ERROR");
    }

    #[tokio::test]
    async fn test_handle_list_logs_limit() {
        let state = create_test_app_state();

        if let Some(broadcaster) = &state.log_broadcaster {
            for i in 0..5 {
                broadcaster.send("INFO", "test", format!("msg {}", i));
            }
        }

        let query = ListLogsQuery { limit: Some(2) };
        let response = handle_list_logs(State(state), Query(query)).await;

        assert!(response.0.success);
        let logs = response.0.data.unwrap();
        assert_eq!(logs.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_log_stream() {
        let state = create_test_app_state();

        let sse = handle_log_stream(State(state)).await;
        let response = sse.into_response();
        let (parts, _body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get("content-type").unwrap(),
            "text/event-stream"
        );
    }

    #[tokio::test]
    async fn test_handle_log_stream_no_broadcaster() {
        let mut state = create_test_app_state();
        state.log_broadcaster = None;

        let sse = handle_log_stream(State(state)).await;
        let response = sse.into_response();
        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::OK);

        // 没有 broadcaster，body 应该为空
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        assert!(bytes.is_empty());
    }
}
