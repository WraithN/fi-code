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

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
}

pub struct LogStore {
    buffer: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        self.buffer
            .iter()
            .rev()
            .take(limit)
            .rev()
            .cloned()
            .collect()
    }
}

pub struct LogBroadcaster {
    tx: broadcast::Sender<LogEntry>,
    store: std::sync::Mutex<LogStore>,
}

impl LogBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self {
            tx,
            store: std::sync::Mutex::new(LogStore::new(capacity)),
        }
    }

    /// 同步方法，供日志宏在非 async 上下文中调用
    pub fn send(&self, level: &str, module: &str, message: String) {
        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            module: module.to_string(),
            message,
        };
        if let Ok(mut store) = self.store.lock() {
            store.push(entry.clone());
        }
        let _ = self.tx.send(entry);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        if let Ok(store) = self.store.lock() {
            store.recent(limit)
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_store_capacity() {
        let mut store = LogStore::new(3);
        store.push(LogEntry {
            timestamp: "00:00:00".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "1".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:01".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "2".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:02".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "3".into(),
        });
        store.push(LogEntry {
            timestamp: "00:00:03".into(),
            level: "INFO".into(),
            module: "a".into(),
            message: "4".into(),
        });
        let recent = store.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].message, "2");
        assert_eq!(recent[2].message, "4");
    }

    #[test]
    fn test_broadcaster_send_and_recent() {
        let b = LogBroadcaster::new(5);
        b.send("INFO", "test", "hello".into());
        let recent = b.recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "hello");
    }
}
