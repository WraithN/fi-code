// =============================================================================
// session 模块：会话持久化与管理
// =============================================================================
// 本模块负责将多轮对话以 JSONL（JSON Lines）格式持久化到本地磁盘，
// 支持会话创建、列表浏览、完整恢复、增量追加写入。
//
// 存储格式设计：
// - 每个 Session 对应一个 `.jsonl` 文件
// - 文件内每行是一个独立的 JSON 记录，支持 append-only 写入和流式恢复
// - 记录类型包括：`session`（文件头）、`message_start`、`part`、`message_end`
// - 采用 ULID 作为 Session ID 和 Message ID，天然可按时间排序

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::message::{current_timestamp_ms, Message, Part, Role};

// =============================================================================
// 会话状态枚举
// =============================================================================

/// 会话生命周期状态。
/// - `Active`：正在活跃使用
/// - `Idle`：超过一定时间无操作（当前由上层逻辑判断）
/// - `Archived`：用户手动归档
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Idle,
    Archived,
}

// =============================================================================
// Session 与 SessionMeta
// =============================================================================

/// 会话根结构，包含会话元数据和完整的消息历史。
#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub messages: Vec<Message>,
}

/// 会话元数据摘要，用于列表展示，避免加载全部消息内容。
#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub project_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub model: String,
    pub status: SessionStatus,
    pub message_count: usize,
}

// =============================================================================
// SessionManager：JSONL 读写核心
// =============================================================================

/// 会话管理器，封装所有与文件系统交互的操作。
/// 内部使用同步 `std::fs` I/O；若需在 async 上下文中调用，
/// 建议通过 `tokio::task::spawn_blocking` 包裹。
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// 创建新的管理器实例。
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// 创建一个新的活跃会话。
    /// - 自动生成 ULID 作为 session_id
    /// - `project_path` 取当前工作目录
    /// - 创建完成后立即写入 `session` 文件头
    pub fn create_session(&self, model: &str) -> Result<Session> {
        fs::create_dir_all(&self.sessions_dir)?;
        let id = ulid::Ulid::new().to_string();
        let now = current_timestamp_ms();
        let project_path = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let session = Session {
            id: id.clone(),
            project_path,
            created_at: now,
            updated_at: now,
            model: model.to_string(),
            status: SessionStatus::Active,
            messages: Vec::new(),
        };
        self.write_session_header(&session)?;
        Ok(session)
    }

    /// 列出 `sessions_dir` 下的所有会话摘要，按 `updated_at` 降序排列。
    /// 遍历 `.jsonl` 文件并调用 `load_session` 读取完整内容后提取元数据。
    pub fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        let mut metas = Vec::new();
        if !self.sessions_dir.exists() {
            return Ok(metas);
        }
        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            // 只处理 .jsonl 扩展名的文件
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(session) = self.load_session(id) {
                        metas.push(SessionMeta {
                            id: session.id,
                            project_path: session.project_path,
                            created_at: session.created_at,
                            updated_at: session.updated_at,
                            model: session.model,
                            status: session.status,
                            message_count: session.messages.len(),
                        });
                    }
                }
            }
        }
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    /// 从 JSONL 文件恢复完整 Session（含所有 Message 和 Part）。
    ///
    /// 恢复逻辑：
    /// 1. 按行读取
    /// 2. 解析为统一的 `Record` 结构
    /// 3. 根据 `type_` 字段分别处理：
    ///    - `session`：初始化 Session 对象
    ///    - `message_start`：创建 MessageBuilder
    ///    - `part`：将 Part 追加到当前 MessageBuilder
    ///    - `message_end`：将 Builder 转为 Message 并压入 Session
    /// 4. 遇到解析失败的行：打印警告并跳过，保证容错性
    pub fn load_session(&self, session_id: &str) -> Result<Session> {
        let path = self.session_path(session_id);
        let file = File::open(&path)
            .with_context(|| format!("Failed to open session file: {:?}", path))?;
        let reader = BufReader::new(file);

        let mut session: Option<Session> = None;
        let mut current_message: Option<MessageBuilder> = None;

        for (line_no, line) in reader.lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Warning: failed to read line {}: {}", line_no + 1, e);
                    continue;
                }
            };
            let record: Record = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Warning: failed to parse line {}: {}", line_no + 1, e);
                    continue;
                }
            };

            match record.type_.as_str() {
                "session" => {
                    session = Some(parse_session_record(record)?);
                }
                "message_start" => {
                    current_message = Some(MessageBuilder::new(record)?);
                }
                "part" => {
                    if let Some(ref mut builder) = current_message {
                        builder.add_part(record)?;
                    }
                }
                "message_end" => {
                    if let Some(builder) = current_message.take() {
                        let msg = builder.finalize(record)?;
                        if let Some(ref mut s) = session {
                            s.messages.push(msg);
                        }
                    }
                }
                _ => {
                    eprintln!("Warning: unknown record type on line {}", line_no + 1);
                }
            }
        }

        session.with_context(|| format!("No session header found in {:?}", path))
    }

    /// 全量覆写保存整个 Session。
    /// 适用场景：初始化重建、批量保存。
    pub fn save_session(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir)?;
        let path = self.session_path(&session.id);
        let mut file = File::create(&path)?;
        // 第一行写入 session 元数据头
        writeln!(file, "{}", serde_json::to_string(&session_to_record(session))?)?;
        // 随后逐条写入消息
        for msg in &session.messages {
            self.write_message(&mut file, msg)?;
        }
        Ok(())
    }

    /// 运行时追加单条 Message（增量持久化）。
    /// 适用场景：用户每输入一条查询或模型每返回一条回复后即时落盘。
    pub fn append_message(&self, session_id: &str, message: &Message) -> Result<()> {
        let path = self.session_path(session_id);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        self.write_message(&mut file, message)?;
        Ok(())
    }

    /// 私有方法：写入 session 文件头（创建新会话时使用）。
    fn write_session_header(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.id);
        let mut file = File::create(&path)?;
        writeln!(file, "{}", serde_json::to_string(&session_to_record(session))?)?;
        Ok(())
    }

    /// 私有方法：将一条 Message 序列化为三行 JSONL：
    /// message_start -> [part...] -> message_end
    fn write_message(&self, file: &mut File, message: &Message) -> Result<()> {
        writeln!(
            file,
            "{}",
            serde_json::to_string(&message_start_record(message))?
        )?;
        for (seq, part) in message.parts.iter().enumerate() {
            writeln!(
                file,
                "{}",
                serde_json::to_string(&part_record(message, seq, part))?
            )?;
        }
        writeln!(
            file,
            "{}",
            serde_json::to_string(&message_end_record(message))?
        )?;
        Ok(())
    }

    /// 构造 session 文件的完整路径：`{sessions_dir}/{session_id}.jsonl`
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.jsonl", session_id))
    }
}

// =============================================================================
// JSONL 记录类型与辅助函数
// =============================================================================

/// 统一的 JSONL 行记录结构。
/// 通过 `#[serde(flatten)]` 将剩余字段存入 `fields` Map，
/// 方便按 `type_` 做二次分发解析。
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    #[serde(rename = "type")]
    type_: String,
    #[serde(flatten)]
    fields: serde_json::Map<String, serde_json::Value>,
}

/// 将 `Session` 元数据转换为 `Record`，用于写入 JSONL 文件头。
fn session_to_record(session: &Session) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("id".to_string(), json!(session.id));
    fields.insert("project_path".to_string(), json!(session.project_path));
    fields.insert("created_at".to_string(), json!(session.created_at));
    fields.insert("updated_at".to_string(), json!(session.updated_at));
    fields.insert("model".to_string(), json!(session.model));
    fields.insert("status".to_string(), serde_json::to_value(&session.status).unwrap());
    Record {
        type_: "session".to_string(),
        fields,
    }
}

/// 从 `Record` 中解析出 `Session`（不含消息内容）。
fn parse_session_record(record: Record) -> Result<Session> {
    Ok(Session {
        id: get_str(&record, "id")?,
        project_path: get_str(&record, "project_path")?,
        created_at: get_u64(&record, "created_at")?,
        updated_at: get_u64(&record, "updated_at")?,
        model: get_str(&record, "model")?,
        status: serde_json::from_value(
            record.fields.get("status").cloned().unwrap_or(json!("active"))
        )?,
        messages: Vec::new(),
    })
}

// =============================================================================
// MessageBuilder：用于从 JSONL 记录流式重建 Message
// =============================================================================

/// 消息构造器，在 `load_session` 过程中暂存一个 Message 的中间状态。
struct MessageBuilder {
    id: String,
    session_id: String,
    role: Role,
    created_at: u64,
    parts: Vec<Part>,
}

impl MessageBuilder {
    fn new(record: Record) -> Result<Self> {
        let role_str = get_str(&record, "role")?;
        Ok(Self {
            id: get_str(&record, "message_id")?,
            session_id: get_str(&record, "session_id").unwrap_or_default(),
            role: serde_json::from_value(json!(role_str))?,
            created_at: get_u64(&record, "created_at")?,
            parts: Vec::new(),
        })
    }

    /// 向当前消息追加一个 Part。
    fn add_part(&mut self, record: Record) -> Result<()> {
        let part_value = record
            .fields
            .get("part")
            .cloned()
            .context("Missing 'part' field")?;
        let part: Part = serde_json::from_value(part_value)?;
        self.parts.push(part);
        Ok(())
    }

    /// 完成消息构造，合并 `message_end` 中可能携带的 token_count 和 cost。
    fn finalize(self, record: Record) -> Result<Message> {
        Ok(Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            created_at: self.created_at,
            parts: self.parts,
            token_count: record.fields.get("token_count").and_then(|v| v.as_u64()),
            cost: record.fields.get("cost").and_then(|v| v.as_f64()),
        })
    }
}

// =============================================================================
// 记录生成辅助函数
// =============================================================================

/// 生成 `message_start` 记录。
fn message_start_record(message: &Message) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    fields.insert("session_id".to_string(), json!(message.session_id));
    fields.insert(
        "role".to_string(),
        serde_json::to_value(&message.role).unwrap(),
    );
    fields.insert("created_at".to_string(), json!(message.created_at));
    Record {
        type_: "message_start".to_string(),
        fields,
    }
}

/// 生成 `part` 记录，`sequence` 保证 Part 的顺序可恢复。
fn part_record(message: &Message, sequence: usize, part: &Part) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    fields.insert("sequence".to_string(), json!(sequence));
    fields.insert("part".to_string(), serde_json::to_value(part).unwrap());
    Record {
        type_: "part".to_string(),
        fields,
    }
}

/// 生成 `message_end` 记录。
fn message_end_record(message: &Message) -> Record {
    let mut fields = serde_json::Map::new();
    fields.insert("message_id".to_string(), json!(message.id));
    if let Some(tc) = message.token_count {
        fields.insert("token_count".to_string(), json!(tc));
    }
    if let Some(c) = message.cost {
        fields.insert("cost".to_string(), json!(c));
    }
    Record {
        type_: "message_end".to_string(),
        fields,
    }
}

/// 从 Record 的 fields 中安全提取字符串。
fn get_str(record: &Record, key: &str) -> Result<String> {
    record
        .fields
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .with_context(|| format!("Missing or invalid field: {}", key))
}

/// 从 Record 的 fields 中安全提取 u64。
fn get_u64(record: &Record, key: &str) -> Result<u64> {
    record
        .fields
        .get(key)
        .and_then(|v| v.as_u64())
        .with_context(|| format!("Missing or invalid field: {}", key))
}

// =============================================================================
// 单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Part, Role};
    use std::io::Write;

    /// 创建临时目录和管理器的辅助函数。
    fn temp_manager() -> (SessionManager, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        (manager, dir)
    }

    /// 测试创建会话后能够正确加载。
    #[test]
    fn test_create_and_load_session() {
        let (manager, _dir) = temp_manager();
        let session = manager.create_session("claude-test").unwrap();
        assert_eq!(session.model, "claude-test");
        assert!(session.messages.is_empty());

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.model, session.model);
    }

    /// 测试追加消息后能够完整恢复，包括 Part 内容。
    #[test]
    fn test_append_and_load_message() {
        let (manager, _dir) = temp_manager();
        let session = manager.create_session("gpt-test").unwrap();

        let msg = Message {
            id: "msg-001".to_string(),
            session_id: session.id.clone(),
            role: Role::User,
            created_at: 1234567890000,
            parts: vec![Part::Text {
                text: "hello world".to_string(),
            }],
            token_count: Some(2),
            cost: Some(0.001),
        };

        manager.append_message(&session.id, &msg).unwrap();

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].id, "msg-001");
        assert_eq!(loaded.messages[0].parts.len(), 1);
        match &loaded.messages[0].parts[0] {
            Part::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("Expected Text part"),
        }
    }

    /// 测试遇到损坏的 JSONL 行时能够跳过并继续恢复后续记录。
    #[test]
    fn test_corrupted_line_skip() {
        let (manager, dir) = temp_manager();
        let session = manager.create_session("test").unwrap();

        // 手动向文件末尾追加一行非法 JSON
        let path = dir.path().join(format!("{}.jsonl", session.id));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "this is not json").unwrap();

        let loaded = manager.load_session(&session.id).unwrap();
        assert_eq!(loaded.messages.len(), 0); // 会话头仍在，消息为空
    }
}
