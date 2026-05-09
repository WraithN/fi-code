# AskForQuestion 工具设计文档

## 概述

AskForQuestion 是一个让 Agent 能向用户提问并获取带选项答案的工具，支持预设选项（最多3个）和用户自定义答案。

## 目标

- Agent 可以调用工具向用户提问
- 提供最多3个预设选项，其中一个是推荐选项
- 支持用户输入自定义答案
- TUI 和桌面应用都显示选择框
- 答案既作为工具调用结果返回，又显示在聊天界面中

## 架构设计

### 数据流向

```
Agent 调用 ask_for_question 工具
    ↓
tool_call 检测工具名 → 发送 AppEvent::ShowQuestionDialog
    ↓
TUI/桌面应用接收事件 → 显示选择框
    ↓
用户选择/输入 → 发送 AppEvent::QuestionAnswered
    ↓
Agent 接收答案 → 作为工具返回值 + 显示在聊天界面
```

## 详细设计

### 1. 工具参数与返回值

#### 参数 JSON Schema

```json
{
  "type": "object",
  "properties": {
    "question": {
      "type": "string",
      "description": "要向用户提出的问题文本"
    },
    "options": {
      "type": "array",
      "maxItems": 3,
      "items": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "description": "选项的唯一标识符"
          },
          "label": {
            "type": "string",
            "description": "选项的显示文本"
          },
          "description": {
            "type": "string",
            "description": "选项的详细说明（可选）"
          }
        },
        "required": ["id", "label"]
      }
    },
    "recommended": {
      "type": "string",
      "description": "推荐选项的 id（可选）"
    },
    "allow_custom": {
      "type": "boolean",
      "default": true,
      "description": "是否允许用户输入自定义答案"
    }
  },
  "required": ["question", "options"]
}
```

#### 返回值格式

工具返回 JSON 字符串：
- 预设选项：`{"type": "option", "id": "a", "label": "方案A"}`
- 自定义答案：`{"type": "custom", "value": "我的自定义答案"}`

### 2. AppEvent 扩展

修改文件：`src/tui/event.rs`

```rust
// 在文件顶部添加
use serde::{Deserialize, Serialize};

// 在 AppEvent 枚举中添加
#[derive(Debug, Clone)]
pub enum AppEvent {
    // ... 现有事件 ...
    ShowQuestionDialog {
        question: String,
        options: Vec<QuestionOption>,
        recommended: Option<String>,
        allow_custom: bool,
    },
    QuestionAnswered {
        answer: QuestionAnswer,
    },
}

// 新增类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestionAnswer {
    Option { id: String, label: String },
    Custom(String),
}
```

### 3. 全局状态与工具注册

修改文件：`src/tools/mod.rs`

#### 3.1 新增全局状态

```rust
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{mpsc, oneshot};
use crate::tui::event::AppEvent;

// 全局事件发送器（TuiApp 初始化时设置）
static EVENT_TX: RwLock<Option<mpsc::Sender<AppEvent>>> = RwLock::new(None);

// 问题答案通道
type QuestionResponseSender = oneshot::Sender<crate::tui::event::QuestionAnswer>;
static QUESTION_CHANNEL: LazyLock<Mutex<Option<QuestionResponseSender>>> = 
    LazyLock::new(|| Mutex::new(None));

// 设置全局事件发送器（供 TuiApp 调用）
pub fn set_event_tx(tx: mpsc::Sender<AppEvent>) {
    let mut event_tx = EVENT_TX.write().unwrap();
    *event_tx = Some(tx);
}
```

#### 3.2 注册 AskForQuestionHandler

```rust
#[derive(Debug)]
struct AskForQuestionHandler;

impl ToolHandler for AskForQuestionHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        // 占位，实际处理在 tool_call 函数中
        Err("AskForQuestion handled in tool_call".to_string())
    }
}

// 在 REGISTRY 初始化中添加
registry
    .register(
        "ask_for_question",
        "Ask the user a question with predefined options",
        r#"{"type":"object","properties":{"question":{"type":"string"},"options":{"type":"array","maxItems":3,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"description":{"type":"string"}},"required":["id","label"]}},"recommended":{"type":"string"},"allow_custom":{"type":"boolean","default":true}},"required":["question","options"]}"#,
        Box::new(AskForQuestionHandler),
    )
    .expect("register ask_for_question failed");
```

#### 3.3 修改 tool_call 函数处理该工具

```rust
pub async fn tool_call(
    name: &str,
    input: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    if name == "ask_for_question" {
        // 解析参数
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or("Missing question parameter")?
            .to_string();

        let options_json = input
            .get("options")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid options parameter")?;

        let options: Vec<crate::tui::event::QuestionOption> = options_json
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        if options.is_empty() || options.len() > 3 {
            return Err("Options count must be between 1 and 3".to_string());
        }

        let recommended = input
            .get("recommended")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let allow_custom = input
            .get("allow_custom")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // 创建 oneshot channel
        let (tx, rx) = oneshot::channel();

        // 保存 sender
        {
            let mut channel = QUESTION_CHANNEL.lock().unwrap();
            *channel = Some(tx);
        }

        // 发送 AppEvent
        if let Some(event_tx) = EVENT_TX.read().unwrap().as_ref() {
            let _ = event_tx.send(AppEvent::ShowQuestionDialog {
                question,
                options,
                recommended,
                allow_custom,
            }).await;
        }

        // 等待用户答案
        match rx.await {
            Ok(answer) => {
                let result = serde_json::to_string(&answer)
                    .map_err(|e| format!("Serialize error: {}", e))?;
                Ok(result)
            }
            Err(_) => Err("No answer received".to_string()),
        }
    }

    // ... 其他工具处理 ...
}
```

### 4. TuiApp 修改

修改文件：`src/tui/app.rs`

在 `TuiApp::new()` 中设置全局事件发送器：

```rust
impl TuiApp {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        // ... 其他初始化代码 ...
        
        // 设置全局事件发送器
        crate::tools::set_event_tx(event_tx.clone());
        
        Self {
            event_tx,
            event_rx,
            // ... 其他字段 ...
        }
    }
}
```

### 4. TUI 实现

修改文件：`src/tui/app.rs`

#### 4.1 新增状态管理

```rust
pub struct TuiApp {
    // ... 现有字段 ...
    pending_question: Option<PendingQuestion>,
}

#[derive(Debug, Clone)]
struct PendingQuestion {
    question: String,
    options: Vec<QuestionOption>,
    recommended: Option<String>,
    allow_custom: bool,
}
```

#### 4.2 修改 handle_event 函数处理新事件

```rust
impl TuiApp {
    fn handle_event(&mut self, event: &AppEvent) {
        // ... 现有处理 ...

        match event {
            AppEvent::ShowQuestionDialog { question, options, recommended, allow_custom } => {
                self.pending_question = Some(PendingQuestion {
                    question: question.clone(),
                    options: options.clone(),
                    recommended: recommended.clone(),
                    allow_custom: *allow_custom,
                });
                self.focus = FocusArea::Main; // 或者新的 FocusArea::Dialog
            }
            AppEvent::QuestionAnswered { answer } => {
                if let Some(tx) = QUESTION_CHANNEL.lock().unwrap().take() {
                    let _ = tx.send(answer.clone());
                }
                
                // 同时将答案添加到聊天消息中
                let answer_text = match answer {
                    QuestionAnswer::Option { label, .. } => label.clone(),
                    QuestionAnswer::Custom(value) => value.clone(),
                };
                self.add_message(crate::session::message::Message::user(&answer_text));
                
                self.pending_question = None;
            }
            // ... 其他事件 ...
        }
    }
}
```

#### 4.3 新增对话框渲染

在 `TuiApp::draw` 函数中添加对话框渲染逻辑。

### 5. 桌面应用实现

桌面应用（Tauri）同样监听 AppEvent 并显示弹窗组件。

## 测试策略

- 单元测试：工具参数解析、事件创建
- 集成测试：完整的问答流程
- 端到端测试：TUI 和桌面应用的 UI 交互

## 风险与注意事项

- 确保 QUESTION_CHANNEL 的线程安全
- 处理用户取消的情况
- 防止答案通道泄漏
