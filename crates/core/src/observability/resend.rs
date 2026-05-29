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

//! observability::resend：启动期扫描 spans.jsonl，识别并标记 pending span。
//!
//! v1 简化实现：仅识别 + 标记 `lf_status="skipped"`（避免下次重复扫描）。
//! 真正的 OTLP 重发交给 v2 增量。原因：opentelemetry_sdk 0.27 暂未暴露
//! 从 JSONL 反序列化回 SpanData 的稳定 API，强行重建会带来不可控的协议风险。
//!
//! 关键不变量：
//! - 文件只读 O_RDONLY；status_patch 通过 LocalJsonlExporter 走主流程写入路径
//! - 末尾 TAIL_LINES 行窗口避免无限扫描
//! - 超过 MAX_AGE 的 pending 视为放弃（不再标 skipped，下次自然被窗口淘汰）

use anyhow::Result;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// 扫描窗口：仅看最近 10000 行，避免长会话累积导致启动卡顿
const TAIL_LINES: usize = 10_000;
// pending 过期阈值：超过 7 天的 pending span 视为放弃
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

// status_patch 行的字段键
const PATCH_TYPE_KEY: &str = "type";
const PATCH_TYPE_VALUE: &str = "status";
const LF_STATUS_KEY: &str = "lf_status";
const LF_STATUS_SENT: &str = "sent";
const LF_STATUS_SKIPPED: &str = "skipped";
const SPAN_IDS_KEY: &str = "span_ids";

// span 行的字段键
const SPAN_ID_KEY: &str = "span_id";
const END_TIME_KEY: &str = "end_time_unix_nano";

/// 启动期运行一次。失败时返回 Err，由 caller 决定如何处理（一般 log_warn 后忽略）。
pub async fn run_once() -> Result<()> {
    // 第 0 步：尝试拿到与导出器共享的 LocalJsonl 句柄；observability 未启用时直接返回
    let local = match crate::observability::tracer::local_exporter() {
        Some(l) => l,
        None => return Ok(()),
    };
    let path = local.path().clone();
    if !path.exists() {
        return Ok(());
    }

    // 第 1 步：读全文，按行倒序取 TAIL_LINES 行（仅扫描尾部窗口）
    let content = std::fs::read_to_string(&path)?;
    let lines: Vec<&str> = content.lines().rev().take(TAIL_LINES).collect();

    // 第 2 步：倒序扫描，聚合 status_patch 行，构建 span_id → 最新 lf_status 映射
    let status_map = build_status_map(&lines);

    // 第 3 步：找出 pending 且未过期的 span_id
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let max_age_ns = MAX_AGE.as_nanos();

    let pending_ids = collect_pending_ids(&lines, &status_map, now_ns, max_age_ns);

    if pending_ids.is_empty() {
        return Ok(());
    }

    crate::log_info!(
        "[observability] resend daemon: identified {} pending spans, marking as skipped (v1; real replay in v2)",
        pending_ids.len()
    );

    // 第 4 步：标记为 skipped，避免下次重复处理
    local.append_status_patch(&pending_ids, LF_STATUS_SKIPPED);
    Ok(())
}

/// 倒序扫描 lines，构建 span_id → 最新 lf_status 映射（"sent" / "skipped" / 其他）。
///
/// 由于 lines 已经是倒序（最新行在前），用 `entry().or_insert` 即可保留最新状态：
/// 第一次写入的就是最新一次 patch，后续旧 patch 不会覆盖。
fn build_status_map(lines: &[&str]) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for line in lines {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // 跳过腐烂行
        };
        if v.get(PATCH_TYPE_KEY).and_then(|t| t.as_str()) != Some(PATCH_TYPE_VALUE) {
            continue;
        }
        let st = v.get(LF_STATUS_KEY).and_then(|s| s.as_str()).unwrap_or("");
        if st.is_empty() {
            continue;
        }
        if let Some(ids) = v.get(SPAN_IDS_KEY).and_then(|i| i.as_array()) {
            for id in ids {
                if let Some(s) = id.as_str() {
                    map.entry(s.to_string()).or_insert_with(|| st.to_string());
                }
            }
        }
    }
    map
}

/// 在 lines 中找出仍 pending（未在 status_map 中标 sent/skipped）且未过期的 span_id。
fn collect_pending_ids(
    lines: &[&str],
    status_map: &HashMap<String, String>,
    now_ns: u128,
    max_age_ns: u128,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // 跳过 status_patch 行本身
        if v.get(PATCH_TYPE_KEY).and_then(|t| t.as_str()) == Some(PATCH_TYPE_VALUE) {
            continue;
        }
        let span_id = match v.get(SPAN_ID_KEY).and_then(|s| s.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        // 已被 sent / skipped 的跳过
        match status_map.get(&span_id).map(String::as_str) {
            Some(LF_STATUS_SENT) | Some(LF_STATUS_SKIPPED) => continue,
            _ => {}
        }
        // 过期检查：end_time 与 now 差距超过 MAX_AGE 则放弃
        let end_ns = v.get(END_TIME_KEY).and_then(|t| t.as_u64()).unwrap_or(0) as u128;
        if end_ns > 0 && now_ns.saturating_sub(end_ns) > max_age_ns {
            continue;
        }
        out.push(span_id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_status_map_aggregates_patches() {
        // lines 已经是倒序（调用方 rev().take(...)）；
        // 这里手工模拟：最前面的行是最新写入的 patch（skipped），后面的是旧的（sent）
        // 因此 span "a" 的最新状态应为 skipped；span "b" 仅出现在旧 patch 中 → sent
        let lines = vec![
            r#"{"type":"status","span_ids":["a"],"lf_status":"skipped","patched_at_unix_nano":3}"#,
            r#"{"type":"status","span_ids":["a","b"],"lf_status":"sent","patched_at_unix_nano":2}"#,
        ];
        let m = build_status_map(&lines);
        assert_eq!(m.get("a").map(String::as_str), Some("skipped"));
        assert_eq!(m.get("b").map(String::as_str), Some("sent"));
    }

    #[test]
    fn test_collect_pending_filters_sent_and_expired() {
        // 构造 now=10 天纳秒；a 的 end_time 设为 now（不过期），b 已 sent，c 的 end_time=1（必过期）
        let now_ns: u128 = 10 * 24 * 3600 * 1_000_000_000;
        let max_age_ns: u128 = 7 * 24 * 3600 * 1_000_000_000;

        let line_a = format!(
            r#"{{"span_id":"a","name":"x","end_time_unix_nano":{}}}"#,
            now_ns
        );
        let line_b = r#"{"span_id":"b","name":"x","end_time_unix_nano":0}"#.to_string();
        let line_c = r#"{"span_id":"c","name":"x","end_time_unix_nano":1}"#.to_string();
        let lines_owned = vec![line_a, line_b, line_c];
        let lines: Vec<&str> = lines_owned.iter().map(|s| s.as_str()).collect();

        let mut status_map: HashMap<String, String> = HashMap::new();
        status_map.insert("b".into(), "sent".into());

        let pending = collect_pending_ids(&lines, &status_map, now_ns, max_age_ns);
        assert!(pending.contains(&"a".to_string()), "a 应作为 pending 收集");
        assert!(!pending.contains(&"b".to_string()), "b 已 sent，应被过滤");
        assert!(!pending.contains(&"c".to_string()), "c 已过期，应被过滤");
    }

    #[test]
    fn test_corrupted_line_skipped() {
        // 腐烂行不应导致 panic；同时合法 span 行应被正常收集
        let lines = vec![
            "this is not json",
            r#"{"span_id":"good","name":"x","end_time_unix_nano":0}"#,
        ];
        let m = build_status_map(&lines);
        assert!(m.is_empty(), "腐烂行 + 无 patch 行 → 映射应为空");
        let pending = collect_pending_ids(&lines, &HashMap::new(), 0, u128::MAX);
        assert_eq!(pending, vec!["good".to_string()]);
    }
}
