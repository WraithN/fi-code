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

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::agent::LoopState;

const SESSION_TIMEOUT: Duration = Duration::from_secs(30 * 60); // 30 分钟

/// HTTP 会话管理器，内存中保存 session_id → LoopState 的映射
pub struct HttpSessionManager {
    sessions: RwLock<HashMap<String, (LoopState, Instant)>>,
}

impl HttpSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// 创建新会话，返回 session_id
    pub fn create(&self) -> String {
        let id = ulid::Ulid::new().to_string();
        let state = LoopState::new(Vec::new());
        self.sessions
            .write()
            .unwrap()
            .insert(id.clone(), (state, Instant::now()));
        id
    }

    /// 获取会话状态（同时刷新时间戳）
    pub fn get(&self, id: &str) -> Option<LoopState> {
        let mut sessions = self.sessions.write().unwrap();
        sessions.get_mut(id).map(|(state, timestamp)| {
            *timestamp = Instant::now();
            LoopState {
                messages: state.messages.clone(),
                turn_count: state.turn_count,
                transition_reason: state.transition_reason.clone(),
                token_usage: state.token_usage,
            }
        })
    }

    /// 保存会话状态
    pub fn save(&self, id: &str, state: LoopState) {
        self.sessions
            .write()
            .unwrap()
            .insert(id.to_string(), (state, Instant::now()));
    }

    /// 清理超时会话
    pub fn cleanup(&self) {
        let now = Instant::now();
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < SESSION_TIMEOUT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let manager = HttpSessionManager::new();
        let id = manager.create();
        assert!(!id.is_empty());

        let state = manager.get(&id);
        assert!(state.is_some());
    }

    #[test]
    fn test_save_and_get() {
        let manager = HttpSessionManager::new();
        let id = manager.create();

        let mut state = LoopState::new(Vec::new());
        state.turn_count = 5;
        manager.save(&id, state);

        let retrieved = manager.get(&id).unwrap();
        assert_eq!(retrieved.turn_count, 5);
    }

    #[test]
    fn test_get_nonexistent() {
        let manager = HttpSessionManager::new();
        assert!(manager.get("nonexistent").is_none());
    }

    #[test]
    fn test_cleanup() {
        let manager = HttpSessionManager::new();
        let id = manager.create();

        // 手动将时间戳设为超时
        {
            let mut sessions = manager.sessions.write().unwrap();
            if let Some((_, timestamp)) = sessions.get_mut(&id) {
                *timestamp = Instant::now() - Duration::from_secs(31 * 60);
            }
        }

        manager.cleanup();
        assert!(manager.get(&id).is_none());
    }
}
