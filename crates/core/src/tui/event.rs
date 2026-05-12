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

use crate::server::transport::sse::SseEvent;
use crate::tui::components::left_drawer::FileNode;
use serde::{Deserialize, Serialize};

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

/// Provider 分组信息（TUI 模型菜单用）。
#[derive(Debug, Clone)]
pub struct ProviderItem {
    pub key: String,
    pub name: String,
    pub provider_type: String,
    pub models: Vec<ModelItem>,
}

/// 模型信息（TUI 模型菜单用）。
#[derive(Debug, Clone)]
pub struct ModelItem {
    pub key: String,
    pub name: String,
    pub context: usize,
    pub output: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Debug,
    Trace,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub timestamp: String,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
}

/// 应用级事件枚举，涵盖用户交互、网络回调、界面控制等所有异步信号。
///
/// 设计意图：将终端输入（`crossterm::Event`）与业务事件解耦，
/// 使组件只需关心自身业务，无需直接处理底层终端细节。
/// 卡片交互动作。
#[derive(Debug, Clone)]
pub enum CardAction {
    Expand(String),    // card_id
    Collapse(String),  // card_id
    Retry(String),     // card_id
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Resize(u16, u16),
    ToggleLeftDrawer,
    CloseDrawers,
    FocusNext,
    FocusPrev,
    SetFocus(FocusArea),
    ToggleModelDropdown,
    ToggleThemeDropdown,
    SelectModel(String),
    SwitchModel {
        provider: String,
        model: String,
        api_key: Option<String>,
    },
    SetModelList(Vec<ProviderItem>),
    SelectModelProvider(String),
    SelectModelItem {
        provider: String,
        model: String,
    },
    SelectTheme(usize),
    PreviewTheme(usize), // 预览主题（方向键移动时触发，未确认）
    CancelThemePreview,  // 取消主题预览（Esc 时恢复原来主题）
    SelectSkill(String), // 确认加载指定 Skill
    NewSession,
    NewSessionWithName(String),
    NewSessionFromTemplate(SessionTemplate),
    SubmitMessage(String),
    InputChanged(String),
    ScrollUp,
    ScrollDown,
    CopyLastCode,
    StopGeneration,
    SseEvent(SseEvent),
    ChatComplete,
    ExecuteComplete(String),
    SwitchSession(String),
    DeleteSession(String),
    RenameSession(String, String),
    ToggleFolder(String),
    SelectFile(String),
    OpenFile(String),
    PreviewFile(String),
    AddToContext(String),
    ClearChat,
    ExecuteSlashCommand {
        name: String,
        args_hint: Option<String>,
    },
    LoadCommands,
    SetCommands(Vec<crate::commands::registry::CommandMeta>),
    ShowSystemMessage(String),
    LoadThemes,
    SetThemes(Vec<crate::tui::theme::ThemePreset>),
    ToggleLogWindow,
    SetLogHistory(Vec<LogLine>),
    AppendLog(LogLine),
    LogDisconnected,
    SetFileTree(Vec<FileNode>),
    ShowQuestionDialog {
        question: String,
        options: Vec<QuestionOption>,
        recommended: Option<String>,
        allow_custom: bool,
    },
    QuestionAnswered {
        answer: QuestionAnswer,
    },
    CardAction(CardAction),
    RetryTurn { turn_index: usize },
    Quit,
}

/// 当前焦点所在的 UI 区域，用于决定键盘事件下发给哪个组件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Main,        // 聊天消息区
    Input,       // 底部输入框
    LeftDrawer,  // 左侧文件抽屉
    RightDrawer, // 右侧会话历史抽屉
}

/// 新建会话时可选择的模板类型。
#[derive(Debug, Clone)]
pub enum SessionTemplate {
    Empty,           // 空白会话
    FromLastContext, // 继承上文
    CodeReview,      // 代码审查模板
    Debug,           // 调试模板
}
