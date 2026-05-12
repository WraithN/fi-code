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
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::server::models::{
    ApiResponse, CreateSessionRequest, RenameSessionRequest, SessionDto, SessionListResponse,
};
use crate::server::server::AppState;

pub async fn list_sessions(
    State(_state): State<AppState>,
) -> Json<ApiResponse<SessionListResponse>> {
    let sessions = vec![];

    let response = SessionListResponse {
        sessions,
        current_session_id: None,
    };

    Json(ApiResponse::success(response))
}

pub async fn create_session(
    State(_state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id: ulid::Ulid::new().to_string(),
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}

pub async fn rename_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: req.name,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: false,
    };

    Json(ApiResponse::success(session))
}

pub async fn delete_session(State(_state): State<AppState>, Path(_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn switch_session(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SessionDto>> {
    let session = SessionDto {
        id,
        name: "switched".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        message_count: 0,
        is_current: true,
    };

    Json(ApiResponse::success(session))
}
