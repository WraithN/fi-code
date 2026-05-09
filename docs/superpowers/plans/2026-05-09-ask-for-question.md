# AskForQuestion 工具实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标**：实现 AskForQuestion 工具，让 Agent 能向用户提问，支持最多3个预设选项和用户自定义答案，TUI 和桌面应用都显示选择框，答案既作为工具结果返回又显示在聊天界面中。

**架构**：使用现有 AppEvent 系统，新增全局状态管理事件发送器和问题答案通道，工具调用时发送 ShowQuestionDialog 事件，TUI/桌面应用显示对话框并在用户选择后返回结果。

**技术栈**：Rust, Tokio, Ratatui, Tauri

---

### Task 1：扩展 AppEvent 类型

**Files**：
- Modify：`/home/nan/fi-code/src/tui/event.rs`

- [ ] **Step 1：在 event.rs 顶部添加 serde 依赖**

```rust
// 在 use 语句部分添加
use serde::{Deserialize, Serialize};
```

- [ ] **Step 2：新增 QuestionOption 和 QuestionAnswer 类型**

```rust
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

- [ ] **Step 3：在 AppEvent 枚举中添加新事件**

```rust
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
```

- [ ] **Step 4：运行 cargo check**

Run：`cargo check`
Expected：Compiles successfully

- [ ] **Step 5：Commit**

```bash
cd /home/nan/fi-code
git add src/tui/event.rs
git commit -m "feat: add AppEvent extensions for AskForQuestion"
```

---

### Task 2：在 tools 模块中添加全局状态

**Files**：
- Modify：`/home/nan/fi-code/src/tools/mod.rs`

- [ ] **Step 1：添加必要的 use 语句**

```rust
use tokio::sync::mpsc;
use crate::tui::event::{AppEvent, QuestionAnswer};
```

- [ ] **Step 2：添加全局状态**

```rust
// 全局事件发送器（TuiApp 初始化时设置）
static EVENT_TX: RwLock<Option<mpsc::Sender<AppEvent>>> = RwLock::new(None);

// 问题答案通道
type QuestionResponseSender = tokio::sync::oneshot::Sender<QuestionAnswer>;
static QUESTION_CHANNEL: LazyLock<Mutex<Option<QuestionResponseSender>>> = 
    LazyLock::new(|| Mutex::new(None));

// 设置全局事件发送器
pub fn set_event_tx(tx: mpsc::Sender<AppEvent>) {
    let mut event_tx = EVENT_TX.write().unwrap();
    *event_tx = Some(tx);
}
```

- [ ] **Step 3：运行 cargo check**

Run：`cargo check`
Expected：Compiles successfully

- [ ] **Step 4：Commit**

```bash
cd /home/nan/fi-code
git add src/tools/mod.rs
git commit -m "feat: add global state for AskForQuestion tool"
```

---

### Task 3：实现 AskForQuestion 工具注册

**Files**：
- Modify：`/home/nan/fi-code/src/tools/mod.rs`

- [ ] **Step 1：添加 AskForQuestionHandler**

```rust
#[derive(Debug)]
struct AskForQuestionHandler;

impl ToolHandler for AskForQuestionHandler {
    fn call(&self, _name: &str, params: ToolParams) -> Result<String, String> {
        Err("AskForQuestion handled in tool_call".to_string())
    }
}
```

- [ ] **Step 2：在 REGISTRY 初始化中注册工具**

```rust
// 在 registry.register(...) 调用链中添加
registry
    .register(
        "ask_for_question",
        "Ask the user a question with predefined options",
        r#"{"type":"object","properties":{"question":{"type":"string"},"options":{"type":"array","maxItems":3,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"description":{"type":"string"}},"required":["id","label"]}},"recommended":{"type":"string"},"allow_custom":{"type":"boolean","default":true}},"required":["question","options"]}"#,
        Box::new(AskForQuestionHandler),
    )
    .expect("register ask_for_question failed");
```

- [ ] **Step 3：运行 cargo check**

Run：`cargo check`
Expected：Compiles successfully

- [ ] **Step 4：Commit**

```bash
cd /home/nan/fi-code
git add src/tools/mod.rs
git commit -m "feat: register AskForQuestion tool"
```

---

### Task 4：实现 tool_call 中的工具处理

**Files**：
- Modify：`/home/nan/fi-code/src/tools/mod.rs`

- [ ] **Step 1：在 tool_call 函数顶部添加 use 语句**

```rust
use crate::tui::event::QuestionOption;
```

- [ ] **Step 2：在 tool_call 函数开头添加 AskForQuestion 处理逻辑**

```rust
pub async fn tool_call(
    name: &str,
    input: &HashMap<String, serde_json::Value>,
) -> Result<String, String> {
    if name == "ask_for_question" {
        let question = input
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or("Missing question parameter")?
            .to_string();

        let options_json = input
            .get("options")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid options parameter")?;

        let options: Vec<QuestionOption> = options_json
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

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channel = QUESTION_CHANNEL.lock().unwrap();
            *channel = Some(tx);
        }

        if let Some(event_tx) = EVENT_TX.read().unwrap().as_ref() {
            let _ = event_tx.send(AppEvent::ShowQuestionDialog {
                question,
                options,
                recommended,
                allow_custom,
            }).await;
        }

        match rx.await {
            Ok(answer) => {
                let result = serde_json::to_string(&answer)
                    .map_err(|e| format!("Serialize error: {}", e))?;
                Ok(result)
            }
            Err(_) => Err("No answer received".to_string()),
        }
    }

    // ... 现有工具处理代码 ...
}
```

- [ ] **Step 3：运行 cargo check**

Run：`cargo check`
Expected：Compiles successfully

- [ ] **Step 4：运行所有测试**

Run：`cargo test`
Expected：All tests pass

- [ ] **Step 5：Commit**

```bash
cd /home/nan/fi-code
git add src/tools/mod.rs
git commit -m "feat: implement AskForQuestion tool call handling"
```

---

### Task 5：在 TuiApp 中设置全局事件发送器

**Files**：
- Modify：`/home/nan/fi-code/src/tui/app.rs`

- [ ] **Step 1：在 TuiApp::new() 中设置全局事件发送器**

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

- [ ] **Step 2：运行 cargo check**

Run：`cargo check`
Expected：Compiles successfully

- [ ] **Step 3：Commit**

```bash
cd /home/nan/fi-code
git add src/tui/app.rs
git commit -m "feat: set global event tx in TuiApp"
```

---

### Task 6：实现 TUI 问题对话框 UI 组件

**Files**：
- Create：`/home/nan/fi-code/src/tui/components/question_dialog.rs`
- Modify：`/home/nan/fi-code/src/tui/components/mod.rs`
- Modify：`/home/nan/fi-code/src/tui/app.rs`

- [ ] **Step 1：创建 question_dialog.rs 文件**

```rust
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

use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use crate::tui::event::{QuestionOption, QuestionAnswer};

#[derive(Debug, Clone)]
enum QuestionDialogAction {
    Submit(QuestionAnswer),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct QuestionDialog {
    question: String,
    options: Vec<QuestionOption>,
    recommended: Option<String>,
    allow_custom: bool,
    selected_index: usize,
    custom_input: String,
    cursor_position: usize,
}

impl QuestionDialog {
    pub fn new(
        question: String,
        options: Vec<QuestionOption>,
        recommended: Option<String>,
        allow_custom: bool,
    ) -> Self {
        Self {
            question,
            options,
            recommended,
            allow_custom,
            selected_index: 0,
            custom_input: String::new(),
            cursor_position: 0,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Option<QuestionDialogAction> {
        match code {
            KeyCode::Enter => {
                if self.is_custom_selected() {
                    Some(QuestionDialogAction::Submit(QuestionAnswer::Custom(self.custom_input.clone())))
                } else if let Some(option) = self.options.get(self.selected_index) {
                    Some(QuestionDialogAction::Submit(QuestionAnswer::Option {
                        id: option.id.clone(),
                        label: option.label.clone(),
                    }))
                } else {
                    None
                }
            }
            KeyCode::Esc => Some(QuestionDialogAction::Cancel),
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }
            KeyCode::Down => {
                let max_index = self.max_index();
                if self.selected_index < max_index {
                    self.selected_index += 1;
                }
                None
            }
            KeyCode::Char(c) if self.is_custom_selected() => {
                self.custom_input.insert(self.cursor_position, c);
                self.cursor_position += 1;
                None
            }
            KeyCode::Backspace if self.is_custom_selected() => {
                if self.cursor_position > 0 {
                    self.custom_input.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
                None
            }
            KeyCode::Left if self.is_custom_selected() => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                None
            }
            KeyCode::Right if self.is_custom_selected() => {
                if self.cursor_position < self.custom_input.len() {
                    self.cursor_position += 1;
                }
                None
            }
            _ => None,
        }
    }

    fn is_custom_selected(&self) -> bool {
        self.allow_custom && self.selected_index == self.options.len()
    }

    fn max_index(&self) -> usize {
        if self.allow_custom {
            self.options.len()
        } else {
            self.options.len() - 1
        }
    }

    pub fn draw(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" 问题 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));
        
        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(inner);

        // 问题文本
        let question_text = Paragraph::new(self.question.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        f.render_widget(question_text, chunks[0]);

        // 选项列表
        let options_area = chunks[1];
        let option_height = 2;
        let option_count = if self.allow_custom {
            self.options.len() + 1
        } else {
            self.options.len()
        };
        let total_height = option_count * option_height;

        let list_items: Vec<ListItem> = self.options.iter().enumerate().map(|(i, option)| {
            let is_recommended = self.recommended.as_ref() == Some(&option.id);
            let label = if is_recommended {
                format!("{} (推荐)", option.label)
            } else {
                option.label.clone()
            };
            let content = if let Some(desc) = &option.description {
                format!("{}\n  {}", label, desc)
            } else {
                label
            };
            let style = if i == self.selected_index {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(content).style(style)
        }).collect();

        let mut items = list_items;
        if self.allow_custom {
            let style = if self.is_custom_selected() {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            items.push(ListItem::new("自定义答案").style(style));
        }

        let list = List::new(items)
            .highlight_symbol("> ");
        f.render_widget(list, options_area);

        // 自定义输入框（如果选中自定义）
        if self.is_custom_selected() {
            let input_area = chunks[2];
            let input = Paragraph::new(self.custom_input.as_str())
                .style(Style::default().bg(Color::DarkGray));
            f.render_widget(input, input_area);
        }
    }
}
```

- [ ] **Step 2：在 components/mod.rs 中导出新组件**

```rust
// 在 pub mod 部分添加
pub mod question_dialog;
```

- [ ] **Step 3：在 TuiApp 中添加对话框状态**

```rust
// 在 TuiApp 结构体中添加字段
use crate::tui::components::question_dialog::QuestionDialog;

pub struct TuiApp {
    // ... 现有字段 ...
    question_dialog: Option<QuestionDialog>,
}
```

- [ ] **Step 4：修改 TuiApp::new() 初始化新字段**

```rust
impl TuiApp {
    pub fn new() -> Self {
        // ... 现有初始化代码 ...
        
        Self {
            // ... 现有字段 ...
            question_dialog: None,
        }
    }
}
```

- [ ] **Step 5：修改 TuiApp::handle_event 处理新事件**

```rust
impl TuiApp {
    fn handle_event(&mut self, event: &AppEvent) {
        // ... 现有处理代码 ...
        
        match event {
            AppEvent::ShowQuestionDialog { question, options, recommended, allow_custom } => {
                self.question_dialog = Some(QuestionDialog::new(
                    question.clone(),
                    options.clone(),
                    recommended.clone(),
                    *allow_custom,
                ));
            }
            // ... 其他事件 ...
            _ => {}
        }
    }
}
```

- [ ] **Step 6：修改 TuiApp 事件循环处理键盘事件**

在事件循环中，当 `question_dialog` 存在时优先处理对话框的键盘事件。

- [ ] **Step 7：修改 TuiApp::draw 渲染对话框**

在 `draw` 函数中，当 `question_dialog` 存在时渲染模态对话框。

- [ ] **Step 8：运行 cargo check 和 cargo test**

Run：`cargo check && cargo test`
Expected：Compiles successfully, all tests pass

- [ ] **Step 9：Commit**

```bash
cd /home/nan/fi-code
git add src/tui/components/question_dialog.rs
git add src/tui/components/mod.rs
git add src/tui/app.rs
git commit -m "feat: add TUI question dialog component"
```

---

### Task 7：实现答案返回与聊天消息添加

**Files**：
- Modify：`/home/nan/fi-code/src/tui/app.rs`

- [ ] **Step 1：处理 QuestionAnswered 事件**

```rust
impl TuiApp {
    fn handle_event(&mut self, event: &AppEvent) {
        // ... 现有代码 ...
        
        match event {
            AppEvent::QuestionAnswered { answer } => {
                // 发送答案到工具通道
                if let Some(tx) = crate::tools::QUESTION_CHANNEL.lock().unwrap().take() {
                    let _ = tx.send(answer.clone());
                }
                
                // 添加用户消息到聊天
                let answer_text = match answer {
                    QuestionAnswer::Option { label, .. } => label.clone(),
                    QuestionAnswer::Custom(value) => value.clone(),
                };
                self.add_message(crate::session::message::Message::user(&answer_text));
                
                // 关闭对话框
                self.question_dialog = None;
            }
            // ... 其他事件 ...
        }
    }
}
```

- [ ] **Step 2：运行 cargo check 和 cargo test**

Run：`cargo check && cargo test`
Expected：Compiles successfully, all tests pass

- [ ] **Step 3：Commit**

```bash
cd /home/nan/fi-code
git add src/tui/app.rs
git commit -m "feat: implement answer handling"
```

---

### Plan Complete!

实现计划已完成！涵盖了从类型扩展到 TUI 组件的所有必要任务。
