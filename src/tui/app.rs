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

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::commands::registry::CommandMeta;
use crate::server::transport::sse::SseEvent;
use crate::tui::components::{
    chat::Chat,
    header::{Header, HeaderStatus},
    input::Input,
    left_drawer::LeftDrawer,
    log_window::LogWindow,
    right_drawer::RightDrawer,
    status_bar::StatusBar,
    question_dialog::{QuestionDialog, QuestionDialogAction},
    Component,
};
use crate::tui::event::{AppEvent, FocusArea, ModelItem, ProviderItem, QuestionAnswer};
use crate::tui::layout::{LayoutManager, PanelState};
use crate::tui::theme::Theme;

use super::client::TuiClient;

/// API Key 输入模态对话框动作。
#[derive(Debug, Clone)]
enum DialogAction {
    Submit(String),
    Cancel,
}

/// API Key 输入模态对话框。
#[derive(Debug, Clone)]
struct ApiKeyDialog {
    provider: String,
    model: String,
    input: String,
    cursor: usize,
}

impl ApiKeyDialog {
    fn new(provider: String, model: String) -> Self {
        Self {
            provider,
            model,
            input: String::new(),
            cursor: 0,
        }
    }

    fn insert(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.input.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    /// 处理键盘事件。
    ///
    /// 返回值：
    /// - `None`：事件已消费，无操作（继续输入）。
    /// - `Some(DialogAction::Submit(api_key))`：用户按 Enter，提交输入（可能为空字符串，表示使用配置中的默认 key）。
    /// - `Some(DialogAction::Cancel)`：用户按 Esc，取消对话框。
    fn handle_key(&mut self, code: KeyCode) -> Option<DialogAction> {
        match code {
            KeyCode::Enter => Some(DialogAction::Submit(self.input.clone())),
            KeyCode::Esc => Some(DialogAction::Cancel),
            KeyCode::Backspace => {
                self.backspace();
                None
            }
            KeyCode::Char(c) => {
                self.insert(c);
                None
            }
            _ => None,
        }
    }
}

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
    theme_presets: Vec<crate::tui::theme::ThemePreset>,
    preview_theme_backup: Option<(usize, Arc<Theme>)>,

    // === 各区域 UI 组件 ===
    header: Header,            // 顶部标题栏（Logo、模型、状态）
    left_drawer: LeftDrawer,   // 左侧文件抽屉
    right_drawer: RightDrawer, // 右侧会话历史抽屉
    chat: Chat,                // 中间聊天消息区
    input: Input,              // 底部输入框
    status_bar: StatusBar,     // 最底部快捷键提示栏
    log_window: LogWindow,     // 日志浮窗

    // === 应用状态 ===
    focus: FocusArea,              // 当前焦点所在区域
    is_generating: bool,           // 是否正在等待模型生成回复
    should_quit: bool,             // 是否退出主循环
    exit_confirm_pending: bool,    // Ctrl+C 是否已按过一次，等待第二次确认退出
    dirty: bool,                   // 是否需要重绘
    api_key_dialog: Option<ApiKeyDialog>, // API Key 输入模态框
    question_dialog: Option<QuestionDialog>, // 问题询问模态框

    // === 后端通信与事件通道 ===
    client: TuiClient,                                      // HTTP 客户端，对接本地 4040 端口服务
    event_tx: mpsc::Sender<AppEvent>,                       // 事件发送端（克隆给异步任务使用）
    event_rx: mpsc::Receiver<AppEvent>,                     // 事件接收端（主循环消费）
    crossterm_rx: mpsc::Receiver<anyhow::Result<Event>>,    // 终端事件接收端（后台线程读取后转发）
}

impl TuiApp {
    /// 创建应用实例，初始化默认主题、布局与各个子组件。
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (crossterm_tx, crossterm_rx) = mpsc::channel(100);

        // 在独立后台线程中持续读取终端事件，避免每次循环都启动 spawn_blocking 任务。
        // 线程在应用退出、channel 被关闭后会自动结束。
        std::thread::spawn(move || {
            loop {
                match crossterm::event::read() {
                    Ok(event) => {
                        if crossterm_tx.blocking_send(Ok(event)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if crossterm_tx.blocking_send(Err(anyhow::anyhow!(e))).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let presets = crate::tui::theme::ThemePreset::all_presets();
        let themes: Vec<Arc<Theme>> = presets
            .iter()
            .map(|p| Arc::new(Theme::from_preset(p)))
            .collect();

        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((80, 24));

        // 设置全局事件发送器，供工具调用时发送事件
        crate::tools::set_event_tx(event_tx.clone());

        Self {
            layout: LayoutManager::new(term_w, term_h),
            theme: themes[0].clone(),
            themes,
            theme_index: 0,
            theme_presets: presets,
            preview_theme_backup: None,
            header: Header::new(),
            left_drawer: LeftDrawer::new(),
            right_drawer: RightDrawer::new(),
            chat: Chat::new(),
            input: Input::new(),
            status_bar: StatusBar::new(),
            log_window: LogWindow::new(),
            focus: FocusArea::Input,
            is_generating: false,
            should_quit: false,
            exit_confirm_pending: false,
            dirty: true,
            api_key_dialog: None,
            question_dialog: None,
            client: TuiClient::new(),
            event_tx,
            event_rx,
            crossterm_rx,
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
            // 只有状态发生变化（dirty）时才执行重绘，避免无意义的 CPU 消耗。
            if self.dirty {
                terminal.draw(|frame| self.draw(frame))?;
                self.dirty = false;
            }

            tokio::select! {
                _ = interval.tick() => self.handle_app_event(AppEvent::Tick).await,
                Some(event) = self.event_rx.recv() => self.handle_app_event(event).await,
                Some(result) = self.crossterm_rx.recv() => self.handle_crossterm_result(result).await,
            }
        }

        Ok(())
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
        let input_extra = if self.header.session_id().is_some() {
            1
        } else {
            0
        };
        let (messages_area, input_area) =
            LayoutManager::split_main(areas.main, input_lines + input_extra);

        self.header.draw(
            frame,
            areas.header,
            &self.theme,
            self.focus == FocusArea::Header,
        );
        self.chat.draw(
            frame,
            messages_area,
            &self.theme,
            self.focus == FocusArea::Main,
        );
        self.input.draw(
            frame,
            input_area,
            &self.theme,
            self.focus == FocusArea::Input,
        );
        self.input.set_last_drawn_area(input_area);
        self.input.update_dropdown_area(input_area);
        self.status_bar
            .draw(frame, areas.status_bar, &self.theme, false);

        if let Some(log_area) = areas.log_window {
            self.log_window.draw(frame, log_area, &self.theme, false);
        }

        if let Some(overlay_area) = areas.overlay {
            let dim = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
            frame.render_widget(dim, areas.main);

            match self.layout.panel {
                PanelState::LeftDrawer => {
                    self.left_drawer.draw(
                        frame,
                        overlay_area,
                        &self.theme,
                        self.focus == FocusArea::LeftDrawer,
                    );
                }
                PanelState::RightDrawer => {
                    self.right_drawer.draw(
                        frame,
                        overlay_area,
                        &self.theme,
                        self.focus == FocusArea::RightDrawer,
                    );
                }
                _ => {}
            }
        } else {
            if let Some(area) = areas.left_drawer {
                self.left_drawer.draw(
                    frame,
                    area,
                    &self.theme,
                    self.focus == FocusArea::LeftDrawer,
                );
            }
            if let Some(area) = areas.right_drawer {
                self.right_drawer.draw(
                    frame,
                    area,
                    &self.theme,
                    self.focus == FocusArea::RightDrawer,
                );
            }
        }

        // 渲染 API Key 输入模态框
        if let Some(ref dialog) = self.api_key_dialog {
            let area = frame.area();
            let dialog_w = 48u16;
            let dialog_h = 6u16;
            let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
            let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
            let dialog_area = ratatui::layout::Rect::new(x, y, dialog_w, dialog_h);

            frame.render_widget(ratatui::widgets::Clear, dialog_area);
            let block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(self.theme.border))
                .style(self.theme.drawer_style());
            let inner = block.inner(dialog_area);
            frame.render_widget(block, dialog_area);

            // Label 与输入框保持同一行，垂直居中于内框
            let label_w = 7u16; // "ApiKey:" 宽度
            let input_w = 24u16.min(inner.width.saturating_sub(label_w + 1));
            let content_y = inner.y + (inner.height.saturating_sub(2)) / 2;

            let label = ratatui::widgets::Paragraph::new("ApiKey:");
            frame.render_widget(
                label,
                ratatui::layout::Rect::new(inner.x, content_y + 1, label_w, 1),
            );

            let input_area = ratatui::layout::Rect::new(inner.x + label_w + 1, content_y, input_w, 2);
            let input_text = if dialog.input.is_empty() {
                " ".to_string()
            } else {
                dialog.input.clone()
            };
            let input_para = ratatui::widgets::Paragraph::new(input_text)
                .style(ratatui::style::Style::default().fg(self.theme.text_primary))
                .block(
                    ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .border_style(ratatui::style::Style::default().fg(self.theme.border)),
                );
            frame.render_widget(input_para, input_area);

            // 将光标焦点放到输入框内（考虑边框偏移）
            let cursor_x = input_area.x + 1 + dialog.cursor as u16;
            let cursor_y = input_area.y + 1;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        // 渲染问题询问模态框
        if let Some(ref dialog) = self.question_dialog {
            let area = frame.area();
            let dialog_w = 60u16.min(area.width.saturating_sub(4));
            let num_options = if dialog.allow_custom {
                dialog.options.len() as u16 + 1
            } else {
                dialog.options.len() as u16
            };
            let dialog_h = (6 + num_options * 2).min(area.height.saturating_sub(4));
            let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
            let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
            let dialog_area = ratatui::layout::Rect::new(x, y, dialog_w, dialog_h);

            frame.render_widget(ratatui::widgets::Clear, dialog_area);
            dialog.draw(frame, dialog_area);
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
            PanelState::None => vec![FocusArea::Main, FocusArea::Input],
            PanelState::LeftDrawer => {
                vec![FocusArea::LeftDrawer, FocusArea::Main, FocusArea::Input]
            }
            PanelState::RightDrawer => {
                vec![FocusArea::Main, FocusArea::Input, FocusArea::RightDrawer]
            }
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
        // 终端输入/鼠标/尺寸事件几乎总是导致 UI 状态变化，直接标记需要重绘。
        self.dirty = true;
        match event {
            Event::Resize(w, h) => self.handle_app_event(AppEvent::Resize(w, h)).await,
            _ => self.route_event(event).await,
        }
    }

    /// 处理 Ctrl 组合键快捷键。
    ///
    /// 映射表：
    /// - `Ctrl+C`：若正在生成则停止生成；否则第一次按提示再按一次，第二次按退出程序。
    /// - `Ctrl+B`：切换左侧文件抽屉。
    /// - `Ctrl+H`：切换右侧会话历史抽屉。
    /// - `Ctrl+M`：打开模型选择子菜单（/models）。
    /// - `Ctrl+N`：打开模型下拉菜单。
    /// - `Ctrl+T`：打开主题选择子菜单（/themes）。
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
                    self.exit_confirm_pending = false;
                } else if self.exit_confirm_pending {
                    self.should_quit = true;
                } else {
                    self.exit_confirm_pending = true;
                    self.chat.add_system_message("Press Ctrl+C again to exit.");
                }
            }
            'b' => {
                self.exit_confirm_pending = false;
                self.handle_app_event(AppEvent::ToggleLeftDrawer).await;
                self.focus = FocusArea::LeftDrawer;
            }
            'h' => {
                self.exit_confirm_pending = false;
                self.handle_app_event(AppEvent::ToggleRightDrawer).await;
                self.focus = FocusArea::RightDrawer;
            }
            'm' => {
                self.exit_confirm_pending = false;
                self.handle_execute_slash_command("models", &None);
            }
            'n' => {
                self.exit_confirm_pending = false;
                let was_open = self.header.has_dropdown_open();
                self.header.toggle_model_dropdown();
                if !was_open && self.header.needs_load_models() {
                    let client = self.client.clone();
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        match client.list_models().await {
                            Ok(data) => {
                                let providers = parse_model_list(&data);
                                let _ = tx.send(AppEvent::SetModelList(providers)).await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(AppEvent::ShowSystemMessage(format!("Load models failed: {}", e)))
                                    .await;
                            }
                        }
                    });
                }
                self.focus = FocusArea::Header;
            }
            't' => {
                self.exit_confirm_pending = false;
                self.handle_execute_slash_command("themes", &None);
            }
            'l' => {
                self.exit_confirm_pending = false;
                self.handle_app_event(AppEvent::ToggleLogWindow).await;
            }
            _ => {
                self.exit_confirm_pending = false;
            }
        }
    }

    /// 处理 Tab / Shift+Tab 焦点切换。
    async fn handle_tab_key(&mut self, key: &crossterm::event::KeyEvent) {
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            self.cycle_focus(false); // Shift+Tab 反向切换
        } else {
            self.cycle_focus(true); // Tab 正向切换
        }
    }

    /// 处理 Esc 键：优先级为关闭日志窗口 > 关闭子菜单 > 关闭抽屉 > 关闭下拉菜单 > 回到 Main 区域。
    fn handle_esc_key(&mut self) -> Option<AppEvent> {
        if self.log_window.is_visible() {
            return Some(AppEvent::ToggleLogWindow);
        }
        if self.input.is_submenu_open() {
            return Some(AppEvent::CancelThemePreview);
        }
        if self.layout.panel != PanelState::None {
            self.layout.close_drawers();
        } else if self.header.has_dropdown_open() {
            self.header.close_dropdowns();
        } else {
            self.focus = FocusArea::Main;
        }
        None
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
        if !matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace
        ) {
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

                // API Key 模态框优先处理键盘事件
                if let Some(ref mut dialog) = self.api_key_dialog {
                    if let Some(action) = dialog.handle_key(key.code) {
                        let provider = dialog.provider.clone();
                        let model = dialog.model.clone();
                        self.api_key_dialog = None;
                        match action {
                            DialogAction::Submit(api_key) => {
                                let api_key_opt = if api_key.is_empty() {
                                    None
                                } else {
                                    Some(api_key)
                                };
                                let client = self.client.clone();
                                let tx = self.event_tx.clone();
                                tokio::spawn(async move {
                                    match client.switch_model(&provider, &model, api_key_opt.as_deref()).await {
                                        Ok(_) => {
                                            let _ = tx.send(AppEvent::SelectModel(model)).await;
                                        }
                                        Err(e) => {
                                            let _ = tx
                                                .send(AppEvent::ShowSystemMessage(format!(
                                                    "Switch model failed: {}",
                                                    e
                                                )))
                                                .await;
                                        }
                                    }
                                });
                            }
                            DialogAction::Cancel => {
                                // 用户取消，仅关闭对话框，不切换模型
                            }
                        }
                    }
                    return;
                }

                // 问题询问模态框处理键盘事件
                if let Some(ref mut dialog) = self.question_dialog {
                    if let Some(action) = dialog.handle_key(key.code) {
                        match action {
                            QuestionDialogAction::Submit(answer) => {
                                let _ = self.event_tx.send(AppEvent::QuestionAnswered { answer }).await;
                            }
                            QuestionDialogAction::Cancel => {
                                self.question_dialog = None;
                            }
                        }
                    }
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
                    if let Some(ev) = self.handle_esc_key() {
                        self.handle_app_event(ev).await;
                    }
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
        // 如果日志窗口可见，先让它处理滚动事件
        if self.log_window.is_visible() {
            if let Some(app_event) = self.log_window.handle_event(&event, true) {
                self.handle_app_event(app_event).await;
                return;
            }
        }

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
        // Tick 仅在生成中时需要重绘（spinner 动画），其余事件默认需要重绘。
        if matches!(event, AppEvent::Tick) {
            if self.is_generating {
                self.dirty = true;
            }
        } else {
            self.dirty = true;
        }

        match event {
            AppEvent::ShowQuestionDialog { ref question, ref options, ref recommended, ref allow_custom } => {
                self.question_dialog = Some(QuestionDialog::new(question.clone(), options.clone(), recommended.clone(), *allow_custom));
            }
            AppEvent::QuestionAnswered { ref answer } => {
                // 发送答案到工具通道
                if let Some(tx) = crate::tools::QUESTION_CHANNEL.lock().unwrap().take() {
                    let _ = tx.send(answer.clone());
                }

                // 添加用户消息到聊天
                let answer_text = match answer {
                    QuestionAnswer::Option { label, .. } => label.clone(),
                    QuestionAnswer::Custom(value) => value.clone(),
                };
                self.chat.add_user_message(&answer_text);

                // 关闭对话框
                self.question_dialog = None;
            }
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
                self.header.set_status(HeaderStatus::Generating);
                self.chat.add_user_message(msg);
                self.start_chat_stream(msg.clone()).await;
            }
            // SSE 事件到达：将内容追加到聊天区；若为 Done 事件则更新会话 ID
            AppEvent::SseEvent(ref sse_event) => {
                self.chat.handle_sse_event(sse_event);
                // 当内容到达时，设置为 Streaming 状态
                match sse_event {
                    SseEvent::Message { .. } | SseEvent::ToolUse { .. } | SseEvent::ToolResult { .. } | SseEvent::MessageDetails { .. } => {
                        self.header.set_status(HeaderStatus::Streaming);
                    }
                    _ => {}
                }
                if let SseEvent::Done { session_id } = sse_event {
                    self.header.set_session_id(session_id.clone());
                    self.input.set_session_id(Some(session_id.clone()));
                }
            }
            AppEvent::ChatComplete => {
                self.is_generating = false;
                self.header.set_status(HeaderStatus::Ready);
            }
            AppEvent::StopGeneration => {
                self.is_generating = false;
                self.header.set_status(HeaderStatus::Ready);
            }
            // 切换左侧文件抽屉：打开时自动将焦点移入，并异步请求当前目录文件树
            AppEvent::ToggleLeftDrawer => {
                self.layout.toggle_left();
                if self.layout.panel == crate::tui::layout::PanelState::LeftDrawer {
                    self.focus = FocusArea::LeftDrawer;
                    let client = self.client.clone();
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        if let Ok(file_tree) = client.get_file_tree(".").await {
                            let files: Vec<crate::tui::components::left_drawer::FileNode> = file_tree
                                .entries
                                .into_iter()
                                .map(|e| crate::tui::components::left_drawer::FileNode {
                                    path: e.path,
                                    name: e.name,
                                    is_dir: e.is_dir,
                                    depth: e.depth,
                                })
                                .collect();
                            let _ = tx.send(AppEvent::SetFileTree(files)).await;
                        }
                    });
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
            AppEvent::SwitchModel { ref provider, ref model, ref api_key } => {
                let client = self.client.clone();
                let tx = self.event_tx.clone();
                let provider = provider.clone();
                let model = model.clone();
                let api_key = api_key.clone();
                tokio::spawn(async move {
                    match client.switch_model(&provider, &model, api_key.as_deref()).await {
                        Ok(_) => {
                            let _ = tx.send(AppEvent::SelectModel(model)).await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AppEvent::ShowSystemMessage(format!(
                                    "Switch model failed: {}",
                                    e
                                )))
                                .await;
                        }
                    }
                });
            }
            AppEvent::SetModelList(ref providers) => {
                self.header.set_providers(providers.clone());
                // 如果当前正在 /models 的 ModelProvider 子菜单中，填充 provider 列表
                if self.input.is_submenu_open() {
                    let items: Vec<(String, String, String)> = providers
                        .iter()
                        .map(|p| (p.key.clone(), p.name.clone(), p.provider_type.clone()))
                        .collect();
                    self.input.set_submenu_items(items);
                }
            }
            AppEvent::SelectModelProvider(ref provider_key) => {
                if let Some(provider) = self.header.get_provider(provider_key) {
                    let items: Vec<(String, String, String)> = provider
                        .models
                        .iter()
                        .map(|m| (m.key.clone(), m.name.clone(), format!("context: {}, output: {}", m.context, m.output)))
                        .collect();
                    self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::ModelList);
                    self.input.set_submenu_context(provider_key.clone());
                    self.input.set_submenu_items(items);
                    self.focus = FocusArea::Input;
                }
            }
            AppEvent::SelectModelItem { ref provider, ref model } => {
                let is_preset = provider != "custom";
                if is_preset {
                    self.api_key_dialog = Some(ApiKeyDialog::new(provider.clone(), model.clone()));
                } else {
                    let client = self.client.clone();
                    let tx = self.event_tx.clone();
                    let provider = provider.clone();
                    let model = model.clone();
                    tokio::spawn(async move {
                        match client.switch_model(&provider, &model, None).await {
                            Ok(_) => {
                                let _ = tx.send(AppEvent::SelectModel(model)).await;
                            }
                            Err(e) => {
                                let _ = tx
                                    .send(AppEvent::ShowSystemMessage(format!(
                                        "Switch model failed: {}",
                                        e
                                    )))
                                    .await;
                            }
                        }
                    });
                }
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
                    let Ok(_) = client.switch_session(&id).await else {
                        return;
                    };
                    let _ = tx.send(AppEvent::ChatComplete).await;
                });
            }
            AppEvent::LoadCommands => {
                self.spawn_load_commands();
            }
            AppEvent::SetCommands(ref commands) => {
                self.input.set_commands(commands.clone());
            }
            AppEvent::ExecuteSlashCommand {
                ref name,
                ref args_hint,
            } => {
                self.handle_execute_slash_command(name, args_hint);
            }
            AppEvent::ShowSystemMessage(ref msg) => {
                self.chat.add_system_message(msg);
            }
            AppEvent::ClearChat => {
                self.chat.clear_messages();
            }
            AppEvent::LoadThemes => {
                self.spawn_load_themes();
            }
            AppEvent::SetThemes(ref presets) => {
                self.theme_presets = presets.clone();
                self.themes = presets
                    .iter()
                    .map(|p| Arc::new(Theme::from_preset(p)))
                    .collect();
                let items: Vec<(String, String, String)> = presets
                    .iter()
                    .map(|p| (p.name.clone(), p.name.clone(), p.description.clone()))
                    .collect();
                self.input.set_submenu_items(items);
                if self.theme_index >= self.themes.len() && !self.themes.is_empty() {
                    self.theme_index = 0;
                    self.theme = self.themes[0].clone();
                }
            }
            AppEvent::PreviewTheme(index) => {
                if index < self.themes.len() {
                    if self.preview_theme_backup.is_none() {
                        self.preview_theme_backup = Some((self.theme_index, self.theme.clone()));
                    }
                    self.theme = self.themes[index].clone();
                }
            }
            AppEvent::SelectTheme(index) => {
                if index < self.themes.len() {
                    self.theme_index = index;
                    self.theme = self.themes[index].clone();
                    self.preview_theme_backup = None;
                    let client = self.client.clone();
                    let theme_name = self.theme_presets[index].name.clone();
                    tokio::spawn(async move {
                        let _ = client
                            .execute_command("themes", Some(theme_name), None)
                            .await;
                    });
                }
                self.input.close_submenu();
            }
            AppEvent::CancelThemePreview => {
                if let Some((idx, theme)) = self.preview_theme_backup.take() {
                    self.theme_index = idx;
                    self.theme = theme;
                }
                self.input.close_submenu();
            }
            AppEvent::SelectSkill(ref name) => {
                self.input.close_submenu();
                match crate::skills::load_skill_content(name) {
                    Ok(content) => {
                        self.chat.add_system_message(&format!(
                            "Skill '{}' loaded.\n\n{}",
                            name, content
                        ));
                    }
                    Err(e) => {
                        self.chat.add_system_message(&format!(
                            "Failed to load skill '{}': {}",
                            name, e
                        ));
                    }
                }
            }
            AppEvent::ToggleLogWindow => {
                let visible = !self.log_window.is_visible();
                self.log_window.set_visible(visible);
                self.layout.log_window = visible;
                if visible {
                    self.spawn_load_logs();
                    self.spawn_log_stream();
                }
            }
            AppEvent::SetLogHistory(ref lines) => {
                self.log_window.set_lines(lines.clone());
            }
            AppEvent::AppendLog(ref line) => {
                self.log_window.append(line.clone());
            }
            AppEvent::LogDisconnected => {
                self.log_window.set_disconnected(true);
            }
            AppEvent::SetFileTree(ref files) => {
                self.left_drawer.set_files(files.clone());
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
        self.log_window.update(&event);
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
                        CommandMeta {
                            name: "clear".into(),
                            description: "Clear conversation".into(),
                            args_hint: None,
                        },
                        CommandMeta {
                            name: "models".into(),
                            description: "Switch model".into(),
                            args_hint: Some("[model_key]".into()),
                        },
                        CommandMeta {
                            name: "init".into(),
                            description: "Generate AGENTS.md".into(),
                            args_hint: None,
                        },
                        CommandMeta {
                            name: "help".into(),
                            description: "Show help".into(),
                            args_hint: None,
                        },
                    ];
                    let _ = tx.send(AppEvent::SetCommands(fallback)).await;
                }
            }
        });
    }

    /// 异步加载日志历史列表。
    fn spawn_load_logs(&self) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            match client.get_logs(200).await {
                Ok(entries) => {
                    let lines: Vec<crate::tui::event::LogLine> = entries
                        .into_iter()
                        .map(|e| crate::tui::event::LogLine {
                            timestamp: e.timestamp,
                            level: match e.level.as_str() {
                                "DEBUG" => crate::tui::event::LogLevel::Debug,
                                "TRACE" => crate::tui::event::LogLevel::Trace,
                                "ERROR" => crate::tui::event::LogLevel::Error,
                                _ => crate::tui::event::LogLevel::Info,
                            },
                            module: e.module,
                            message: e.message,
                        })
                        .collect();
                    let _ = tx.send(AppEvent::SetLogHistory(lines)).await;
                }
                Err(_) => {}
            }
        });
    }

    /// 订阅日志 SSE 实时流。
    fn spawn_log_stream(&self) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            if let Err(_) = client.subscribe_logs(tx.clone()).await {
                let _ = tx.send(AppEvent::LogDisconnected).await;
            }
        });
    }

    /// 异步加载主题列表。
    fn spawn_load_themes(&self) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            match client.list_themes().await {
                Ok(presets) => {
                    let _ = tx.send(AppEvent::SetThemes(presets)).await;
                }
                Err(_) => {}
            }
        });
    }

    /// 处理斜杠命令执行：有参数时等待补全，无参数时直接执行。
    /// /theme 命令特殊处理：直接进入主题子菜单。
    fn handle_execute_slash_command(&mut self, name: &str, _args_hint: &Option<String>) {
        if name == "skills" {
            let registry = crate::skills::get_registry();
            if registry.entries.is_empty() {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let _ = tx.send(AppEvent::ShowSystemMessage(
                        "No skills available.".into(),
                    )).await;
                });
                return;
            }
            self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Skill);
            let items: Vec<(String, String, String)> = registry
                .entries
                .iter()
                .map(|e| (e.metadata.name.clone(), e.metadata.name.clone(), e.metadata.description.clone()))
                .collect();
            self.input.set_submenu_items(items);
            return;
        }

        if name == "themes" {
            self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::Theme);
            let items: Vec<(String, String, String)> = self
                .theme_presets
                .iter()
                .map(|p| (p.name.clone(), p.name.clone(), p.description.clone()))
                .collect();
            self.input.set_submenu_items(items);
            self.spawn_load_themes();
            return;
        }

        if name == "models" {
            self.input.enter_submenu_mode(crate::tui::components::input::SubmenuKind::ModelProvider);
            let client = self.client.clone();
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                match client.list_models().await {
                    Ok(data) => {
                        let providers = parse_model_list(&data);
                        let _ = tx.send(AppEvent::SetModelList(providers)).await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AppEvent::ShowSystemMessage(format!("Load models failed: {}", e)))
                            .await;
                    }
                }
            });
            return;
        }

        self.input.set_content(format!("/{}", name));
        if _args_hint.is_some() {
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
                            if let Some(model) = meta.get("current_model").and_then(|v| v.as_str())
                            {
                                let _ = tx.send(AppEvent::SelectModel(model.to_string())).await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AppEvent::ShowSystemMessage(format!("Error: {}", e)))
                            .await;
                    }
                }
            });
            self.input.clear_content();
        }
    }
}


/// 解析后端 /api/models 返回的 JSON，转换为 ProviderItem 列表。
fn parse_model_list(data: &serde_json::Value) -> Vec<crate::tui::event::ProviderItem> {
    let mut providers = Vec::new();
    let Some(arr) = data.get("providers").and_then(|v| v.as_array()) else {
        return providers;
    };
    for p in arr {
        let key = p.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let provider_type = p.get("type").and_then(|v| v.as_str()).unwrap_or("openai_compatible").to_string();
        let mut models = Vec::new();
        if let Some(m_arr) = p.get("models").and_then(|v| v.as_array()) {
            for m in m_arr {
                let m_key = m.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let m_name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let limit = m.get("limit").and_then(|v| v.as_object());
                let context = limit.and_then(|l| l.get("context")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let output = limit.and_then(|l| l.get("output")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                models.push(crate::tui::event::ModelItem {
                    key: m_key,
                    name: m_name,
                    context,
                    output,
                });
            }
        }
        providers.push(crate::tui::event::ProviderItem {
            key,
            name,
            provider_type,
            models,
        });
    }
    providers
}
