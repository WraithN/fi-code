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
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::components::Component;
use crate::tui::event::AppEvent;
use crate::tui::theme::Theme;

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

/// 底部状态栏组件，显示品牌、进度条、耗时、Token 统计和当前模型。
///
/// 该组件不可聚焦，仅作为信息展示。
pub struct StatusBar {
    progress_state: ProgressState,
    progress_tick: u64, // 动画帧计数器
    last_filled: usize, // 暂停时定格的填充格数
    model_name: String, // 当前模型名
    token_in: usize,    // 输入 Token 计数
    token_out: usize,   // 输出 Token 计数
    elapsed_secs: u64,  // 当前耗时（秒）
}

/// 进度条总格数。
const PROGRESS_BAR_WIDTH: usize = 20;

impl StatusBar {
    pub fn new() -> Self {
        Self {
            progress_state: ProgressState::Idle,
            progress_tick: 0,
            last_filled: 0,
            model_name: "unknown".to_string(),
            token_in: 0,
            token_out: 0,
            elapsed_secs: 0,
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

    /// 更新 Token 计数。
    pub fn set_tokens(&mut self, in_count: usize, out_count: usize) {
        self.token_in = in_count;
        self.token_out = out_count;
    }

    /// 更新耗时（秒）。
    pub fn set_elapsed(&mut self, secs: u64) {
        self.elapsed_secs = secs;
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
                let seed = self.progress_tick.wrapping_mul(1103515245).wrapping_add(12345);
                ((seed % 19) + 1) as usize
            }
            ProgressState::Paused => self.last_filled,
        }
    }

    /// 渲染进度条字符串。
    fn render_progress_bar(&self) -> String {
        let filled = self.current_filled();
        let empty = PROGRESS_BAR_WIDTH - filled;
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    }

    /// 格式化耗时显示。
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

    /// 构建状态栏完整显示行。
    fn build_line(&self, theme: &Theme) -> Line<'static> {
        let mut spans = vec![];

        // 品牌标识：FiCode（品牌色）
        spans.push(Span::styled("FiCode", theme.style_brand()));
        spans.push(Span::raw("  "));

        // 进度条
        let progress_bar = self.render_progress_bar();
        let progress_style = match self.progress_state {
            ProgressState::Idle => Style::default().fg(theme.success), // 完成：绿色
            ProgressState::Running => Style::default().fg(theme.warning), // 进行中：黄色
            ProgressState::Paused => Style::default().fg(theme.success), // 完成：绿色
        };
        spans.push(Span::styled(progress_bar, progress_style));

        // 分隔符 + 耗时
        let elapsed = self.format_elapsed();
        if !elapsed.is_empty() {
            spans.push(Span::styled(" ｜ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("耗时：{}", elapsed),
                theme.style_primary(),
            ));
        }

        // 分隔符 + Token 统计
        if self.token_in > 0 || self.token_out > 0 {
            spans.push(Span::styled(" ｜ ", theme.style_muted()));
            spans.push(Span::styled(
                format!("IN:{} OUT:{}", self.token_in, self.token_out),
                theme.style_primary(),
            ));
        }

        // 分隔符 + 模型名
        spans.push(Span::styled(" ｜ ", theme.style_muted()));
        spans.push(Span::styled(
            format!("Model:{}", self.model_name),
            theme.style_primary(),
        ));

        Line::from(spans)
    }
}

impl Component for StatusBar {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme, _is_focused: bool) {
        let line = self.build_line(theme);
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
    fn test_progress_bar_idle() {
        let bar = StatusBar::new();
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[░░░░░░░░░░░░░░░░░░░░]");
    }

    #[test]
    fn test_progress_bar_running() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        bar.on_tick(); // tick = 1, filled = 10
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[██████████░░░░░░░░░░]");

        // 前进到 tick = 5, filled = 9
        for _ in 0..4 {
            bar.on_tick();
        }
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█████████░░░░░░░░░░░]");
    }

    #[test]
    fn test_progress_bar_capped_at_width() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        // tick=40 时 filled = 5
        for _ in 0..40 {
            bar.on_tick();
        }
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█████░░░░░░░░░░░░░░░]");

        // tick=42 时 filled = 14
        bar.on_tick();
        bar.on_tick();
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[██████████████░░░░░░]");
    }

    #[test]
    fn test_progress_bar_paused() {
        let mut bar = StatusBar::new();
        bar.set_generating(true);
        for _ in 0..5 {
            bar.on_tick();
        }
        // tick=5, filled=9
        bar.set_generating(false); // 暂停，定格在 9 格

        // 即使继续 tick，也不应前进
        bar.on_tick();
        bar.on_tick();
        let pb = bar.render_progress_bar();
        assert_eq!(pb, "[█████████░░░░░░░░░░░]");
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
    fn test_build_line_includes_model() {
        let mut bar = StatusBar::new();
        bar.set_model("kimi-code".to_string());
        let theme = Theme::deep_ocean();
        let line = bar.build_line(&theme);
        let text = line.to_string();
        assert!(text.contains("FiCode"));
        assert!(text.contains("Model:kimi-code"));
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
        assert!(row_text.contains("FiCode"), "Status bar should show FiCode brand");
        assert!(
            row_text.contains("Model:unknown"),
            "Status bar should show default model"
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
        bar.set_elapsed(65);

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
        assert!(row_text.contains("Model:gpt-4"));
        assert!(row_text.contains("IN:100 OUT:200"));
        assert!(row_text.contains("1m5s"));
        // 运行状态下进度条不应全是空格
        assert!(row_text.contains('█'));
    }
}
