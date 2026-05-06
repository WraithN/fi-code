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

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::commands::registry::CommandMeta;
use crate::server::sse::SseEvent;
use crate::tui::components::{
    chat::Chat, header::Header, input::Input, left_drawer::LeftDrawer, right_drawer::RightDrawer,
    status_bar::StatusBar, Component,
};
use crate::tui::event::{AppEvent, FocusArea};
use crate::tui::layout::{LayoutManager, PanelState};
use crate::tui::theme::Theme;

use super::client::TuiClient;

/// TUI 应用主结构体，负责统筹所有组件、事件循环与后端通信。
///
/// 内部采用生产者-消费者模型处理事件：
/// - `event_tx` / `event_rx`：跨线程/异步任务发送事件（如 SSE 流、HTTP 回调）。
/// - 主循环通过 `tokio::select!` 同时监听定时 tick、应用事件和终端输入。
pub struct TuiApp {
    layout: LayoutManager,
    theme: Arc<Theme>,
    themes: Vec<Arc<Theme>>,
    theme_index: usize,

    // === 各区域 UI 组件 ===
    header: Header,           // 顶部标题栏（Logo、模型、状态）
    left_drawer: LeftDrawer,  // 左侧文件抽屉
    right_drawer: RightDrawer,// 右侧会话历史抽屉
    chat: Chat,               // 中间聊天消息区
    input: Input,             // 底部输入框
    status_bar: StatusBar,    // 最底部快捷键提示栏

    // === 应用状态 ===
    focus: FocusArea,         // 当前焦点所在区域
    is_generating: bool,      // 是否正在等待模型生成回复
    should_quit: bool,        // 是否退出主循环

    // === 后端通信与事件通道 ===
    client: TuiClient,                    // HTTP 客户端，对接本地 4040 端口服务
    event_tx: mpsc::Sender<AppEvent>,     // 事件发送端（克隆给异步任务使用）
    event_rx: mpsc::Receiver<AppEvent>,   // 事件接收端（主循环消费）
}

impl TuiApp {
    /// 创建应用实例，初始化默认主题、布局与各个子组件。
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let themes = vec![
            Arc::new(Theme::deep_ocean()),
            Arc::new(Theme::github_dark()),
        ];

        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));

        Self {
            layout: LayoutManager::new(term_w, term_h),
            theme: themes[0].clone(),
            themes,
            theme_index: 0,
            header: Header::new(),
            left_drawer: LeftDrawer::new(),
            right_drawer: RightDrawer::new(),
            chat: Chat::new(),
            input: Input::new(),
            status_bar: StatusBar::new(),
            focus: FocusArea::Input,
            is_generating: false,
            should_quit: false,
            client: TuiClient::new(),
            event_tx,
            event_rx,
        }
    }

    /// TUI 主循环。
    ///
    /// 每帧执行顺序：
    /// 1. `terminal.draw`：根据当前状态渲染全部组件。
    /// 2. `tokio::select!`：等待以下三类事件：
    ///    - 每 80ms 的 `Tick`：用于更新动画（如 spinner）。
    ///    - 异步任务通过 `event_rx` 发来的应用事件（如 SSE 消息到达、会话切换完成）。
    ///    - 终端键盘/鼠标事件（通过 `crossterm` 读取）。
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        // 启动时先请求后端状态，获取当前使用的模型名并显示在标题栏
        if let Ok(model) = self.client.get_status().await {
            self.header.set_current_model(model);
        }

        let mut interval = tokio::time::interval(Duration::from_millis(80));

        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
            _ = interval.tick() => self.handle_app_event(AppEvent::Tick).await,
            Some(event) = self.event_rx.recv() => self.handle_app_event(event).await,
            result = Self::read_crossterm_event() => self.handle_crossterm_result(result).await,
        }
        }

        Ok(())
    }

    /// 在独立阻塞线程中读取终端事件，避免阻塞 tokio 异步运行时。
    ///
    /// 使用 `event::poll` 实现 100ms 超时：若超时则返回 Err，让外层 `tokio::select!`
    /// 可以继续处理其他事件（如 Tick 或应用事件）。
    async fn read_crossterm_event() -> anyhow::Result<Event> {
        tokio::task::spawn_blocking(|| {
            if event::poll(Duration::from_millis(100))? {
                Ok(event::read()?)
            } else {
                Err(anyhow::anyhow!("timeout"))
            }
        })
        .await?
    }

    /// 渲染一帧画面。
    ///
    /// 布局层级：
    /// 1. 根据终端尺寸计算整体 `LayoutAreas`（header、main、status_bar、可选 drawer）。
    /// 2. 将 `main` 区域进一步切分为消息区（上方）和输入区（下方）。
    /// 3. 若处于窄屏模式且抽屉打开，则在 main 上方覆盖一层 overlay 渲染抽屉。
    /// 4. 正常宽屏模式下，左右抽屉与 main 并排显示。
    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        self.layout.resize(area.width, area.height);
        let areas = self.layout.calculate();
        let input_lines = self.input.visible_lines();
        // 如果有会话 ID，给输入框额外加一行显示
        let input_extra = if self.header.session_id().is_some() { 1 } else { 0 };
        let (messages_area, input_area) = LayoutManager::split_main(areas.main, input_lines + input_extra);

        self.header.draw(frame, areas.header, &self.theme, self.focus == FocusArea::Header);
        self.chat.draw(frame, messages_area, &self.theme, self.focus == FocusArea::Main);
        self.input.draw(frame, input_area, &self.theme, self.focus == FocusArea::Input);
        self.input.set_last_drawn_area(input_area);
        self.input.update_dropdown_area(input_area);
        self.status_bar.draw(frame, areas.status_bar, &self.theme, false);

        if let Some(overlay_area) = areas.overlay {
            let dim = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
            frame.render_widget(dim, areas.main);

            match self.layout.panel {
                PanelState::LeftDrawer => {
                    self.left_drawer.draw(frame, overlay_area, &self.theme, self.focus == FocusArea::LeftDrawer);
                }
                PanelState::RightDrawer => {
                    self.right_drawer.draw(frame, overlay_area, &self.theme, self.focus == FocusArea::RightDrawer);
                }
                _ => {}
            }
        } else {
            if let Some(area) = areas.left_drawer {
                self.left_drawer.draw(frame, area, &self.theme, self.focus == FocusArea::LeftDrawer);
            }
            if let Some(area) = areas.right_drawer {
                self.right_drawer.draw(frame, area, &self.theme, self.focus == FocusArea::RightDrawer);
            }
        }
    }

    /// 切换到下一套配色主题（循环）。
    fn next_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % self.themes.len();
        self.theme = self.themes[self.theme_index].clone();
    }

    /// 在可用焦点区域之间循环切换。
    ///
    /// 可用区域会根据当前抽屉状态动态变化：
    /// - 无抽屉时：Main ↔ Input
    /// - 左侧抽屉打开时：LeftDrawer ↔ Main ↔ Input
    /// - 右侧抽屉打开时：Main ↔ Input ↔ RightDrawer
    fn cycle_focus(&mut self, forward: bool) {
        let areas = match self.layout.panel {
            PanelState::None => vec![
                FocusArea::Main,
                FocusArea::Input,
            ],
            PanelState::LeftDrawer => vec![
                FocusArea::LeftDrawer,
                FocusArea::Main,
                FocusArea::Input,
            ],
            PanelState::RightDrawer => vec![
                FocusArea::Main,
                FocusArea::Input,
                FocusArea::RightDrawer,
            ],
        };

        let current_idx = areas.iter().position(|a| a == &self.focus).unwrap_or(0);
        let next_idx = if forward {
            (current_idx + 1) % areas.len()
        } else {
            (current_idx + areas.len() - 1) % areas.len()
        };

        self.focus = areas[next_idx];
    }

    /// 处理从 `crossterm` 读取到的终端事件。
    ///
    /// - `Resize` 直接转换为应用事件。
    /// - 其他事件进入路由分发流程。
    async fn handle_crossterm_result(&mut self, result: anyhow::Result<Event>) {
        let Ok(event) = result else { return };
        match event {
            Event::Resize(w, h) => self.handle_app_event(AppEvent::Resize(w, h)).await,
            _ => self.route_event(event).await,
        }
    }

    /// 处理 Ctrl 组合键快捷键。
    ///
    /// 映射表：
    /// - `Ctrl+C`：若正在生成则停止生成，否则退出程序。
    /// - `Ctrl+B`：切换左侧文件抽屉。
    /// - `Ctrl+H`：切换右侧会话历史抽屉。
    /// - `Ctrl+M` / `Ctrl+N`：打开模型下拉菜单。
    /// - `Ctrl+T`：切换主题。
    async fn handle_ctrl_key(&mut self, key: &crossterm::event::KeyEvent) {
        let KeyCode::Char(c) = key.code else { return };
        // crossterm 对 Ctrl+字母 的字符编码为 ASCII 控制字符（如 Ctrl+C = 0x03），
        // 需要将其还原为可读的字母以便匹配。
        let lower = if c.is_ascii_control() {
            (c as u8 + b'a' - 1) as char
        } else {
            c.to_ascii_lowercase()
        };
        match lower {
            'c' => {
                if self.is_generating {
                    self.handle_app_event(AppEvent::StopGeneration).await;
                } else {
                    self.should_quit = true;
                }
            }
            'b' => {
                self.handle_app_event(AppEvent::ToggleLeftDrawer).await;
                self.focus = FocusArea::LeftDrawer;
            }
            'h' => {
                self.handle_app_event(AppEvent::ToggleRightDrawer).await;
                self.focus = FocusArea::RightDrawer;
            }
            'm' | 'n' => {
                self.header.toggle_model_dropdown();
                self.focus = FocusArea::Header;
            }
            't' => self.next_theme(),
            _ => {}
        }
    }

    /// 处理 Tab / Shift+Tab 焦点切换。
    async fn handle_tab_key(&mut self, key: &crossterm::event::KeyEvent) {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            self.cycle_focus(false); // Shift+Tab 反向切换
        } else {
            self.cycle_focus(true);  // Tab 正向切换
        }
    }

    /// 处理 Esc 键：优先级为关闭抽屉 > 关闭下拉菜单 > 回到 Main 区域。
    fn handle_esc_key(&mut self) {
        if self.layout.panel != PanelState::None {
            self.layout.close_drawers();
        } else if self.header.has_dropdown_open() {
            self.header.close_dropdowns();
        } else {
            self.focus = FocusArea::Main;
        }
    }

    /// 当用户在 Main（聊天区）直接按键时，自动将焦点转移到输入框。
    ///
    /// 这样用户无需先按 Tab 切到输入框即可直接打字。
    fn maybe_focus_input(&mut self, event: &Event) {
        if self.focus != FocusArea::Main {
            return;
        }
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press || !key.modifiers.is_empty() {
            return;
        }
        if !matches!(key.code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace) {
            return;
        }
        self.focus = FocusArea::Input;
    }

    /// 事件路由：将终端事件按类型分发到对应的处理函数。
    ///
    /// 处理优先级（从高到低）：
    /// 1. 非按键事件（如鼠标）直接下发给组件。
    /// 2. 只处理 `Press` 阶段，忽略 `Repeat` / `Release`。
    /// 3. Ctrl 组合键 → 全局快捷键。
    /// 4. Tab → 焦点循环。
    /// 5. Esc → 关闭/回退。
    /// 6. 其余按键下发给当前焦点组件。
    async fn route_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.handle_ctrl_key(&key).await;
                    return;
                }

                if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.handle_tab_key(&key).await;
                    return;
                }

                if key.code == KeyCode::Esc && key.modifiers.is_empty() {
                    self.handle_esc_key();
                    return;
                }

                self.maybe_focus_input(&Event::Key(key));
                self.dispatch_event(Event::Key(key)).await;
            }
            Event::Mouse(mouse) => {
                if self.focus == FocusArea::Input {
                    let app_event = self.input.handle_event(&Event::Mouse(mouse), true);
                    if let Some(app_event) = app_event {
                        self.handle_app_event(app_event).await;
                    }
                }
            }
            _ => {
                self.maybe_focus_input(&event);
                self.dispatch_event(event).await;
            }
        }
    }

    /// 将终端事件下发给当前获得焦点的组件。
    ///
    /// 若组件返回 `Some(AppEvent)`，说明组件产生了更高层级的应用事件（如提交消息、切换会话），
    /// 需要再交给 `handle_app_event` 统一处理。
    async fn dispatch_event(&mut self, event: Event) {
        let app_event = match self.focus {
            FocusArea::Header => self.header.handle_event(&event, true),
            FocusArea::LeftDrawer => self.left_drawer.handle_event(&event, true),
            FocusArea::RightDrawer => self.right_drawer.handle_event(&event, true),
            FocusArea::Main => self.chat.handle_event(&event, true),
            FocusArea::Input => self.input.handle_event(&event, true),
        };

        if let Some(app_event) = app_event {
            self.handle_app_event(app_event).await;
        }
    }

    /// 应用事件的核心处理函数，所有组件产生的 `AppEvent` 最终都汇聚到这里。
    ///
    /// 处理完成后会同步各组件状态（如生成状态、面板状态）。
    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                self.chat.on_tick();
                self.header.on_tick();
            }
            AppEvent::Resize(w, h) => {
                self.layout.resize(w, h);
            }
            // 用户提交消息：标记生成中、添加到聊天区，并启动 SSE 流请求
            AppEvent::SubmitMessage(ref msg) => {
                self.is_generating = true;
                self.chat.add_user_message(msg);
                self.start_chat_stream(msg.clone()).await;
            }
            // SSE 事件到达：将内容追加到聊天区；若为 Done 事件则更新会话 ID
            AppEvent::SseEvent(ref sse_event) => {
                self.chat.handle_sse_event(sse_event);
                if let SseEvent::Done { session_id } = sse_event {
                    self.header.set_session_id(session_id.clone());
                    self.input.set_session_id(Some(session_id.clone()));
                }
            }
            AppEvent::ChatComplete => {
                self.is_generating = false;
            }
            AppEvent::StopGeneration => {
                self.is_generating = false;
            }
            // 切换左侧文件抽屉：打开时自动将焦点移入，并异步请求当前目录文件树
            AppEvent::ToggleLeftDrawer => {
                self.layout.toggle_left();
                if self.layout.panel == crate::tui::layout::PanelState::LeftDrawer {
                    self.focus = FocusArea::LeftDrawer;
                    let client = self.client.clone();
                    tokio::spawn(async move { let _ = client.get_file_tree(".").await; });
                }
            }
            AppEvent::ToggleRightDrawer => {
                self.layout.toggle_right();
                if self.layout.panel == crate::tui::layout::PanelState::RightDrawer {
                    self.focus = FocusArea::RightDrawer;
                }
            }
            AppEvent::CloseDrawers => {
                self.layout.close_drawers();
            }
            AppEvent::SelectModel(ref model) => {
                self.header.set_current_model(model.clone());
            }
            AppEvent::SelectTheme(index) => {
                if index < self.themes.len() {
                    self.theme_index = index;
                    self.theme = self.themes[index].clone();
                }
            }
            // 切换会话：异步调用后端接口，完成后发送 ChatComplete 事件通知主循环
            AppEvent::SwitchSession(ref id) => {
                let client = self.client.clone();
                let tx = self.event_tx.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    let Ok(_) = client.switch_session(&id).await else { return };
                    let _ = tx.send(AppEvent::ChatComplete).await;
                });
            }
            AppEvent::LoadCommands => {
                self.spawn_load_commands();
            }
            AppEvent::SetCommands(ref commands) => {
                self.input.set_commands(commands.clone());
            }
            AppEvent::ExecuteSlashCommand { ref name, ref args_hint } => {
                self.handle_execute_slash_command(name, args_hint);
            }
            AppEvent::ShowSystemMessage(ref msg) => {
                self.chat.add_system_message(msg);
            }
            AppEvent::ClearChat => {
                self.chat.clear_messages();
            }
            _ => {}
        }

        // 同步底部状态栏的生成状态与面板状态
        self.status_bar.set_generating(self.is_generating);
        self.status_bar.set_panel(self.layout.panel);

        self.header.update(&event);
        self.chat.update(&event);
        self.input.update(&event);
        self.left_drawer.update(&event);
        self.right_drawer.update(&event);
        self.status_bar.update(&event);
    }

    /// 启动聊天 SSE 流。
    ///
    /// 为了避免阻塞主循环，该函数会在新 tokio 任务中：
    /// 1. 调用 `client.chat` 建立 SSE 连接。
    /// 2. 使用内部 channel (`sse_tx`/`sse_rx`) 将收到的每个 SSE 事件转发到主事件通道。
    /// 3. 流结束后发送 `ChatComplete`；若出错则发送 `SseEvent::Error`。
    async fn start_chat_stream(&self, message: String) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        let session_id = self.header.session_id();

        tokio::spawn(async move {
            let (sse_tx, mut sse_rx) = mpsc::channel(100);

            // 转发任务：将 SSE channel 中的事件桥接到主事件通道
            let forward_handle = {
                let tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(event) = sse_rx.recv().await {
                        let _ = tx.send(AppEvent::SseEvent(event)).await;
                    }
                })
            };

            match client.chat(session_id, message, sse_tx).await {
                Ok(_) => {
                    let _ = forward_handle.await;
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
                Err(e) => {
                    let _ = forward_handle.await;
                    let _ = tx
                        .send(AppEvent::SseEvent(SseEvent::Error {
                            message: e.to_string(),
                        }))
                        .await;
                    let _ = tx.send(AppEvent::ChatComplete).await;
                }
            }
        });
    }

    /// 异步加载命令列表，失败时回退到硬编码命令。
    fn spawn_load_commands(&self) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            match client.list_commands().await {
                Ok(commands) => {
                    let _ = tx.send(AppEvent::SetCommands(commands)).await;
                }
                Err(_) => {
                    let fallback = vec![
                        CommandMeta { name: "clear".into(), description: "Clear conversation".into(), args_hint: None },
                        CommandMeta { name: "model".into(), description: "Switch model".into(), args_hint: Some("[model_key]".into()) },
                        CommandMeta { name: "init".into(), description: "Generate AGENTS.md".into(), args_hint: None },
                        CommandMeta { name: "help".into(), description: "Show help".into(), args_hint: None },
                    ];
                    let _ = tx.send(AppEvent::SetCommands(fallback)).await;
                }
            }
        });
    }

    /// 处理斜杠命令执行：有参数时等待补全，无参数时直接执行。
    fn handle_execute_slash_command(&mut self, name: &str, args_hint: &Option<String>) {
        self.input.set_content(format!("/{}", name));
        if args_hint.is_some() {
            self.input.set_cursor_position(self.input.content().len());
            self.input.close_dropdown();
        } else {
            let client = self.client.clone();
            let tx = self.event_tx.clone();
            let session_id = self.header.session_id();
            let cmd_name = name.to_string();
            tokio::spawn(async move {
                match client.execute_command(&cmd_name, None, session_id).await {
                    Ok(output) => {
                        if !matches!(output.r#type, crate::commands::registry::OutputType::Silent) {
                            let _ = tx.send(AppEvent::ShowSystemMessage(output.message)).await;
                        }
                        if let Some(meta) = output.metadata {
                            if let Some(model) = meta.get("current_model").and_then(|v| v.as_str()) {
                                let _ = tx.send(AppEvent::SelectModel(model.to_string())).await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::ShowSystemMessage(format!("Error: {}", e))).await;
                    }
                }
            });
            self.input.clear_content();
        }
    }
}
