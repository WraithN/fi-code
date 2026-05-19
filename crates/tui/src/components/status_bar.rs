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

use crossterm::event::Event;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::components::Component;
use crate::theme::Theme;
use fi_code_shared::tui_event::AppEvent;

/// 进度条动画状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressState {
    /// 空闲：进度条为空，静态显示。
    Idle,
    /// 运行中：每 tick 前进一格，到头后停在最满状态。
    Running,
    /// 暂停：定格在当前长度，不再前进。
    Paused,
}

/// 底部状态栏组件，显示品牌、CTX 进度条、Token 统计、Latency 和当前模型。
///
/// 该组件不可聚焦，仅作为信息展示。
pub struct StatusBar {
    progress_state: ProgressState,
    progress_tick: u64, // 动画帧计数器
    last_filled: usize, // 暂停时定格的填充格数
    model_name: String, // 当前模型名
    agent_name: String, // 当前 Agent 名称
    token_in: usize,    // 输入 Token 计数
    token_out: usize,   // 输出 Token 计数
    elapsed_secs: u64,  // 当前耗时（秒），保留用于向后兼容
    ctx_current: usize, // 当前上下文 Token 数
    ctx_limit: usize,   // 上下文窗口上限
    latency_ms: u32,    // 上次请求延迟（毫秒）
    is_compressing: bool,
    compression_progress: u8,
}

use fi_code_shared::constants::*;
// PROGRESS_BAR_WIDTH, CTX_BAR_WIDTH, DEFAULT_CTX_LIMIT 已从 fi_code_shared::constants 导入

impl StatusBar {
    pub fn new() -> Self {
        Self {
            progress_state: ProgressState::Idle,
            progress_tick: 0,
            last_filled: 0,
            model_name: "unknown".to_string(),
            agent_name: "Build".to_string(),
            token_in: 0,
            token_out: 0,
            elapsed_secs: 0,
            ctx_current: 0,
            ctx_limit: DEFAULT_CTX_LIMIT,
            latency_ms: 0,
            is_compressing: false,
            compression_progress: 0,
        }
    }

    /// 更新生成状态。
    pub fn set_generating(&mut self, generating: bool) {
        match (self.progress_state, generating) {
            (ProgressState::Idle, true) => {
                self.progress_state = ProgressState::Running;
                self.progress_tick = 0;
                self.last_filled = 0;
            }
            (ProgressState::Running, false) => {
                let filled = self.current_filled();
                self.progress_state = ProgressState::Paused;
                self.last_filled = filled;
            }
            (ProgressState::Paused, true) => {
                self.progress_state = ProgressState::Running;
                self.progress_tick = self.last_filled as u64;
            }
            (ProgressState::Paused, false) => {
                // 已经是暂停状态，重置为空闲
                self.progress_state = ProgressState::Idle;
                self.progress_tick = 0;
                self.last_filled = 0;
            }
            _ => {}
        }
    }

    /// 更新当前模型名。
    pub fn set_model(&mut self, model: String) {
        self.model_name = model;
    }

    /// 更新当前 Agent 名称。
    pub fn set_agent(&mut self, agent_name: String) {
        self.agent_name = agent_name;
    }

    /// 更新 Token 计数。
    pub fn set_tokens(&mut self, in_count: usize, out_count: usize) {
        self.token_in = in_count;
        self.token_out = out_count;
    }

    /// 更新耗时（秒）。
    pub fn set_elapsed(&mut self, secs: u64) {
        self.elapsed_secs = secs;
    }

    /// 更新上下文 Token 数与上限。
    pub fn set_ctx_tokens(&mut self, current: usize, limit: usize) {
        self.ctx_current = current;
        self.ctx_limit = limit;
    }

    /// 返回上下文窗口上限。
    pub fn ctx_limit(&self) -> usize {
        self.ctx_limit
    }

    /// 设置压缩状态。
    pub fn set_compressing(&mut self, compressing: bool) {
        self.is_compressing = compressing;
    }

    /// 设置压缩进度。
    pub fn set_compression_progress(&mut self, progress: u8) {
        self.compression_progress = progress;
    }

    /// 更新上次请求延迟。
    pub fn set_latency(&mut self, latency_ms: u32) {
        self.latency_ms = latency_ms;
    }

    /// 每帧 tick，更新进度条动画。
    pub fn on_tick(&mut self) {
        if self.progress_state == ProgressState::Running {
            self.progress_tick = self.progress_tick.wrapping_add(1);
        }
    }

    /// 计算当前应填充的格数。
    /// Running 状态下每 tick 随机 1~19 格，只要没返回就持续有动态效果。
    fn current_filled(&self) -> usize {
        match self.progress_state {
            ProgressState::Idle => 0,
            ProgressState::Running => {
                // 使用 progress_tick 作为随机种子，每 tick 生成 1~19 的随机值
                let seed = self
                    .progress_tick
                    .wrapping_mul(1103515245)
                    .wrapping_add(12345);
                ((seed % 19) + 1) as usize
            }
            ProgressState::Paused => self.last_filled,
        }
    }

    /// 渲染旧版 20 格进度条字符串（保留用于向后兼容）。
    #[allow(dead_code)]
    fn render_progress_bar(&self) -> String {
        let filled = self.current_filled();
        let empty = PROGRESS_BAR_WIDTH - filled;
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    }

    /// 渲染 CTX 进度条字符串（10 格）。
    /// 始终按实际上下文占用率计算；压缩中显示压缩进度。
    fn render_ctx_bar(&self) -> String {
        let ratio = if self.ctx_limit == 0 {
            0.0
        } else {
            (self.ctx_current as f64 / self.ctx_limit as f64).min(1.0)
        };
        let filled = ((ratio * CTX_BAR_WIDTH as f64).ceil() as usize).min(CTX_BAR_WIDTH);
        let empty = CTX_BAR_WIDTH - filled;
        let pct = (ratio * 100.0) as u8;

        if self.is_compressing {
            let c_filled = ((self.compression_progress as f64 / 100.0) * CTX_BAR_WIDTH as f64)
                .ceil() as usize;
            let c_empty = CTX_BAR_WIDTH - c_filled;
            format!("[{}{}] 🗜️", "█".repeat(c_filled), "░".repeat(c_empty))
        } else {
            format!("[{}{}] {}%", "█".repeat(filled), "░".repeat(empty), pct)
        }
    }

    /// 根据上下文占用率返回进度条颜色。
    fn ctx_bar_style(&self, theme: &Theme) -> Style {
        let ratio = if self.ctx_limit > 0 {
            self.ctx_current as f64 / self.ctx_limit as f64
        } else {
            0.0
        };
        let color = if ratio > 0.85 {
            theme.error
        } else if ratio > 0.60 {
            theme.warning
        } else {
            theme.success
        };
        Style::default().fg(color)
    }

    /// 格式化耗时显示（保留用于向后兼容）。
    #[allow(dead_code)]
    fn format_elapsed(&self) -> String {
        if self.elapsed_secs == 0 {
            String::new()
        } else {
            let minutes = self.elapsed_secs / 60;
            let secs = self.elapsed_secs % 60;
            if minutes > 0 {
                format!("{}m{}s", minutes, secs)
            } else {
                format!("{}s", secs)
            }
        }
    }

    /// 格式化延迟显示。
    fn format_latency(&self) -> String {
        if self.latency_ms == 0 {
            String::new()
        } else {
            let secs = self.latency_ms as f64 / 1000.0;
            format!("{:.1}s", secs)
        }
    }

    /// 根据延迟返回样式。
    fn latency_style(&self, theme: &Theme) -> Style {
        let secs = self.latency_ms as f64 / 1000.0;
        let color = if secs > 30.0 {
            theme.error
        } else if secs > 10.0 {
            theme.warning
        } else {
            theme.text_primary
        };
        Style::default().fg(color)
    }

    /// 格式化 Token 数为紧凑字符串。
    fn format_tokens(n: usize) -> String {
        if n >= 1_000_000 {
            format!("{}M", n / 1_000_000)
        } else if n >= 1_000 {
            format!("{}k", n / 1_000)
        } else {
            format!("{}", n)
        }
    }

    /// 返回当前本地时间 HH:MM。
    fn current_time() -> String {
        chrono::Local::now().format("%H:%M").to_string()
    }

    /// 返回模型短名（用于紧凑模式）。
    fn short_model_name(&self) -> String {
        if self.model_name.len() <= 8 {
            self.model_name.clone()
        } else if let Some(idx) = self.model_name.rfind('-') {
            let suffix = &self.model_name[idx + 1..];
            if suffix.len() <= 8 {
                suffix.to_string()
            } else {
                self.model_name.chars().take(8).collect()
            }
        } else {
            self.model_name.chars().take(8).collect()
        }
    }

    /// 构建状态栏完整显示行。
    fn build_line(&self, theme: &Theme, width: u16) -> Line<'static> {
        let mut spans = vec![];

        // 品牌标识：FiCode（品牌色 + 粗体）
        spans.push(Span::styled(
            "FiCode",
            theme.style_brand().add_modifier(Modifier::BOLD),
        ));

        let ctx_bar = self.render_ctx_bar();
        let ctx_style = self.ctx_bar_style(theme);

        if width >= 100 {
            // ===== 标准模式 =====
            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("AGT: {}", self.agent_name),
                theme.style_primary(),
            ));

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled("CTX: ", theme.style_primary()));
            spans.push(Span::styled(ctx_bar, ctx_style));

            // 空间充裕时显示具体数值
            let ctx_text = format!(
                " {}/{}",
                Self::format_tokens(self.ctx_current),
                Self::format_tokens(self.ctx_limit)
            );
            spans.push(Span::styled(ctx_text, theme.style_muted()));

            if self.token_in > 0 || self.token_out > 0 {
                spans.push(Span::styled(" │ ", theme.style_muted()));
                let tok_text = format!(
                    "TOK: ↑{} ↓{}",
                    Self::format_tokens(self.token_in),
                    Self::format_tokens(self.token_out)
                );
                spans.push(Span::styled(tok_text, theme.style_primary()));
            }

            if self.latency_ms > 0 {
                spans.push(Span::styled(" │ ", theme.style_muted()));
                spans.push(Span::styled(
                    format!("LAT: {}", self.format_latency()),
                    self.latency_style(theme),
                ));
            }

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("MDL: {}", self.model_name),
                Style::default().fg(theme.success),
            ));

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(Self::current_time(), theme.style_muted()));
        } else if width >= 80 {
            // ===== 紧凑模式 =====
            spans.push(Span::raw(" "));
            spans.push(Span::styled(ctx_bar, ctx_style));

            if self.token_out > 0 {
                spans.push(Span::styled(" │ ", theme.style_muted()));
                spans.push(Span::styled(
                    format!("TOK:↓{}", Self::format_tokens(self.token_out)),
                    theme.style_primary(),
                ));
            }

            if self.latency_ms > 0 {
                spans.push(Span::styled(" │ ", theme.style_muted()));
                spans.push(Span::styled(
                    format!("LAT:{}", self.format_latency()),
                    self.latency_style(theme),
                ));
            }

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(
                self.short_model_name(),
                Style::default().fg(theme.success),
            ));

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(Self::current_time(), theme.style_muted()));
        } else {
            // ===== 极限模式 =====
            spans.push(Span::raw(" "));
            spans.push(Span::styled(ctx_bar, ctx_style));

            if self.latency_ms > 0 {
                spans.push(Span::styled(" │ ", theme.style_muted()));
                spans.push(Span::styled(
                    format!("LAT:{}", self.format_latency()),
                    self.latency_style(theme),
                ));
            }

            spans.push(Span::styled(" │ ", theme.style_muted()));
            spans.push(Span::styled(
                self.short_model_name(),
                Style::default().fg(theme.success),
            ));
        }

        Line::from(spans)
    }
}

impl Component for StatusBar {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let line = self.build_line(theme, area.width);
        let paragraph = Paragraph::new(line).style(theme.status_bar_style());
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _focus: bool) -> Option<AppEvent> {
        None
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn update(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Tick => self.on_tick(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_initial_state() {
        let bar = StatusBar::new();
        assert_eq!(bar.progress_state, ProgressState::Idle);
        assert_eq!(bar.progress_tick, 0);
        assert_eq!(bar.model_name, "unknown");
        assert_eq!(bar.agent_name, "Build");
        assert_eq!(bar.ctx_limit, DEFAULT_CTX_LIMIT);
        assert_eq!(bar.ctx_current, 0);
        assert_eq!(bar.latency_ms, 0);
    }

    #[test]
    fn test_status_bar_agent() {
        let mut bar = StatusBar::new();
        assert_eq!(bar.agent_name, "Build");
        bar.set_agent("Plan".to_string());
        assert_eq!(bar.agent_name, "Plan");
    }

    #[test]
    fn test_progress_state_transitions() {
        let mut bar = StatusBar::new();

        // 空闲 → 运行
        bar.set_generating(true);
        assert_eq!(bar.progress_state, ProgressState::Running);

        // 运行 → 暂停
        bar.set_generating(false);
        assert_eq!(bar.progress_state, ProgressState::Paused);

        // 暂停 → 空闲（再次停止）
        bar.set_generating(false);
        assert_eq!(bar.progress_state, ProgressState::Idle);

        // 空闲 → 运行 → 运行（无变化）
        bar.set_generating(true);
        bar.set_generating(true);
        assert_eq!(bar.progress_state, ProgressState::Running);
    }

    #[test]
    fn test_ctx_bar_idle() {
        let bar = StatusBar::new();
        let pb = bar.render_ctx_bar();
        assert_eq!(pb, "[░░░░░░░░░░] 0%");
    }

    #[test]
    fn test_ctx_bar_with_usage() {
        let mut bar = StatusBar::new();
        bar.set_ctx_tokens(64_000, 128_000); // 50%
        let pb = bar.render_ctx_bar();
        assert_eq!(pb, "[█████░░░░░] 50%");
    }

    #[test]
    fn test_ctx_bar_running() {
        // Running 状态下仍按 ctx 占用率计算，不再使用动画
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        bar.set_ctx_tokens(64_000, 128_000); // 50%
        let pb = bar.render_ctx_bar();
        assert_eq!(pb, "[█████░░░░░] 50%");
    }

    #[test]
    fn test_ctx_bar_compressing() {
        let mut bar = StatusBar::new();
        bar.set_ctx_tokens(108_000, 128_000); // ~84%
        bar.set_compressing(true);
        bar.set_compression_progress(45);
        let pb = bar.render_ctx_bar();
        assert_eq!(pb, "[█████░░░░░] 🗜️");
    }

    #[test]
    fn test_ctx_bar_paused() {
        let mut bar = StatusBar::new();
        bar.set_generating(false);
        bar.set_ctx_tokens(32_000, 128_000); // 25%
        let pb = bar.render_ctx_bar();
        assert_eq!(pb, "[███░░░░░░░] 25%");
    }

    #[test]
    fn test_elapsed_format() {
        let mut bar = StatusBar::new();
        assert_eq!(bar.format_elapsed(), "");

        bar.set_elapsed(52);
        assert_eq!(bar.format_elapsed(), "52s");

        bar.set_elapsed(125);
        assert_eq!(bar.format_elapsed(), "2m5s");
    }

    #[test]
    fn test_format_latency() {
        let mut bar = StatusBar::new();
        assert_eq!(bar.format_latency(), "");

        bar.set_latency(2400);
        assert_eq!(bar.format_latency(), "2.4s");

        bar.set_latency(30000);
        assert_eq!(bar.format_latency(), "30.0s");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(StatusBar::format_tokens(500), "500");
        assert_eq!(StatusBar::format_tokens(24000), "24k");
        assert_eq!(StatusBar::format_tokens(1800000), "1M");
    }

    #[test]
    fn test_short_model_name() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-k2.5".to_string());
        assert_eq!(bar.short_model_name(), "k2.5");

        bar.set_model("gpt-4o".to_string());
        assert_eq!(bar.short_model_name(), "gpt-4o");

        bar.set_model("claude-3-7-sonnet-20250219".to_string());
        assert_eq!(bar.short_model_name(), "20250219");
    }

    #[test]
    fn test_build_line_standard_mode() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-k2.5".to_string());
        bar.set_tokens(24000, 18000);
        bar.set_ctx_tokens(64000, 128000);
        bar.set_latency(2400);

        let theme = Theme::deep_ocean();
        let line = bar.build_line(&theme, 120);
        let text = line.to_string();

        assert!(text.contains("FiCode"), "should show brand");
        assert!(
            text.contains("CTX:"),
            "should show CTX label in standard mode"
        );
        assert!(
            text.contains("64k/128k"),
            "should show ctx ratio when space permits"
        );
        assert!(text.contains("TOK:"), "should show TOK");
        assert!(text.contains("↑24k"), "should show input tokens");
        assert!(text.contains("↓18k"), "should show output tokens");
        assert!(text.contains("LAT: 2.4s"), "should show latency");
        assert!(
            text.contains("MDL: kimi-k2.5"),
            "should show full model name"
        );
    }

    #[test]
    fn test_build_line_compact_mode() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-k2.5".to_string());
        bar.set_tokens(24000, 18000);
        bar.set_ctx_tokens(64000, 128000);
        bar.set_latency(2400);

        let theme = Theme::deep_ocean();
        let line = bar.build_line(&theme, 90);
        let text = line.to_string();

        assert!(text.contains("FiCode"), "should show brand");
        assert!(
            !text.contains("CTX:"),
            "should NOT show CTX label in compact mode"
        );
        assert!(
            text.contains("TOK:↓18k"),
            "should show only output tokens in compact mode"
        );
        assert!(
            !text.contains("↑24k"),
            "should NOT show input tokens in compact mode"
        );
        assert!(text.contains("LAT:2.4s"), "should show latency");
        assert!(text.contains("k2.5"), "should show short model name");
        assert!(
            !text.contains("MDL:"),
            "should NOT show MDL label in compact mode"
        );
    }

    #[test]
    fn test_build_line_extreme_mode() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-k2.5".to_string());
        bar.set_tokens(24000, 18000);
        bar.set_ctx_tokens(64000, 128000);
        bar.set_latency(2400);

        let theme = Theme::deep_ocean();
        let line = bar.build_line(&theme, 60);
        let text = line.to_string();

        assert!(text.contains("FiCode"), "should show brand");
        assert!(text.contains("LAT:2.4s"), "should show latency");
        assert!(text.contains("k2.5"), "should show short model name");
        assert!(
            !text.contains("TOK:"),
            "should NOT show TOK in extreme mode"
        );
        assert!(
            !text.contains("10:"),
            "should NOT show clock in extreme mode"
        );
    }

    #[test]
    fn test_ctx_bar_color_transitions() {
        let theme = Theme::deep_ocean();

        let mut bar = StatusBar::new();
        bar.set_ctx_tokens(10_000, 128_000); // < 60%
        let style = bar.ctx_bar_style(&theme);
        assert_eq!(style.fg, Some(theme.success));

        bar.set_ctx_tokens(80_000, 128_000); // 60-85%
        let style = bar.ctx_bar_style(&theme);
        assert_eq!(style.fg, Some(theme.warning));

        bar.set_ctx_tokens(110_000, 128_000); // > 85%
        let style = bar.ctx_bar_style(&theme);
        assert_eq!(style.fg, Some(theme.error));
    }

    #[test]
    fn test_latency_color_transitions() {
        let theme = Theme::deep_ocean();

        let mut bar = StatusBar::new();
        bar.set_latency(5000); // < 10s
        let style = bar.latency_style(&theme);
        assert_eq!(style.fg, Some(theme.text_primary));

        bar.set_latency(15000); // 10-30s
        let style = bar.latency_style(&theme);
        assert_eq!(style.fg, Some(theme.warning));

        bar.set_latency(35000); // > 30s
        let style = bar.latency_style(&theme);
        assert_eq!(style.fg, Some(theme.error));
    }

    // =============================================================================
    // TestBackend 渲染快照测试
    // =============================================================================

    #[test]
    fn test_render_idle_status_bar() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::deep_ocean();
        let bar = StatusBar::new();

        terminal
            .draw(|f| {
                bar.draw(f, f.area(), &theme, false);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        // 验证缓冲区第 0 行包含 FiCode 品牌名
        let row_text: String = (0..buffer.area().width)
            .map(|x| buffer.get(x, 0).symbol().to_string())
            .collect();
        assert!(
            row_text.contains("FiCode"),
            "Status bar should show FiCode brand"
        );
    }

    #[test]
    fn test_render_running_status_bar() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::deep_ocean();
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        bar.set_model("gpt-4".to_string());
        bar.set_tokens(100, 200);
        bar.set_ctx_tokens(1000, 128000);
        bar.set_latency(2400);

        terminal
            .draw(|f| {
                bar.draw(f, f.area(), &theme, false);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let row_text: String = (0..buffer.area().width)
            .map(|x| buffer.get(x, 0).symbol().to_string())
            .collect();
        assert!(row_text.contains("FiCode"));
        assert!(row_text.contains("gpt-4"));
        assert!(row_text.contains("TOK:↓200") || row_text.contains("TOK:"));
        assert!(row_text.contains("LAT:2.4s"));
        // 运行状态下进度条不应全是空格
        assert!(row_text.contains('█'));
    }
}
