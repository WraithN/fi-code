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

use ratatui::layout::Rect;

/// 面板状态：仅控制左侧边栏开关，右侧边栏始终常驻。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    LeftClosed,
    LeftOpen,
}

/// 布局管理器，负责根据终端尺寸计算各组件的 `Rect` 区域。
///
/// 支持两种模式：
/// - 宽屏（≥80 列）：抽屉与主区域并排。
/// - 窄屏（<80 列）：抽屉以 overlay 浮层形式覆盖在主区域上方。
pub struct LayoutManager {
    pub terminal_size: (u16, u16), // (宽, 高)
    pub panel: PanelState,         // 当前左侧面板状态
    pub narrow_mode: bool,         // 是否为窄屏模式
    pub log_window: bool,          // 是否显示日志窗口
}

/// 计算出的各区域坐标与尺寸。
#[derive(Debug)]
pub struct LayoutAreas {
    pub left_drawer: Option<Rect>,
    pub main: Rect,
    pub right_drawer: Rect, // 改为非 Option，右侧边栏始终常驻
    pub status_bar: Rect,
    pub overlay: Option<Rect>,    // 窄屏模式下的抽屉浮层
    pub log_window: Option<Rect>, // 日志窗口
}

impl LayoutManager {
    /// 创建布局管理器，初始左侧面板关闭，并根据宽度判定是否进入窄屏模式。
    pub fn new(width: u16, height: u16) -> Self {
        let narrow_mode = width < 80;
        Self {
            terminal_size: (width, height),
            panel: PanelState::LeftClosed,
            narrow_mode,
            log_window: false,
        }
    }

    /// 终端尺寸变化时更新内部状态。
    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
        self.narrow_mode = width < 80;
    }

    /// 切换左侧抽屉（在 LeftClosed 和 LeftOpen 之间切换）。
    pub fn toggle_left(&mut self) {
        self.panel = match self.panel {
            PanelState::LeftOpen => PanelState::LeftClosed,
            PanelState::LeftClosed => PanelState::LeftOpen,
        };
    }

    /// 关闭左侧抽屉。
    pub fn close_left(&mut self) {
        self.panel = PanelState::LeftClosed;
    }

    /// 根据当前状态计算每个组件应占据的 `Rect`。
    ///
    /// 固定行高：status_bar = 1，剩余为 main 区域。
    pub fn calculate(&self) -> LayoutAreas {
        let (width, height) = self.terminal_size;
        let status_height = 1u16;
        let main_height = height.saturating_sub(status_height);

        let right_width = ((width as f32 * 0.28) as u16).clamp(24, 40);

        if self.narrow_mode && self.panel == PanelState::LeftOpen {
            // 窄屏模式下左侧边栏以浮层形式覆盖
            let overlay_width = (width as f32 * 0.75).max(30.0).min(width as f32) as u16;
            let overlay_x = 0;

            let mut main = Rect::new(
                0,
                0,
                width.saturating_sub(right_width),
                main_height,
            );
            let log_window = if self.log_window {
                let log_height = (main.height as f32 * 0.6) as u16;
                main.height = main.height.saturating_sub(log_height);
                Some(Rect::new(
                    main.x,
                    main.y + main.height,
                    main.width,
                    log_height,
                ))
            } else {
                None
            };

            LayoutAreas {
                left_drawer: None,
                main,
                right_drawer: Rect::new(
                    width.saturating_sub(right_width),
                    0,
                    right_width,
                    main_height,
                ),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                overlay: Some(Rect::new(
                    overlay_x,
                    0,
                    overlay_width,
                    main_height,
                )),
                log_window,
            }
        } else {
            let left_width = ((width as f32 * 0.22) as u16).clamp(20, 35);
            let left_open = self.panel == PanelState::LeftOpen;

            let main_width = if left_open {
                width.saturating_sub(left_width + right_width)
            } else {
                width.saturating_sub(right_width)
            };

            let left_x = 0;
            let main_x = if left_open { left_width } else { 0 };
            let right_x = main_x + main_width;

            let mut main = Rect::new(main_x, 0, main_width, main_height);
            let log_window = if self.log_window {
                let log_height = (main.height as f32 * 0.6) as u16;
                main.height = main.height.saturating_sub(log_height);
                Some(Rect::new(
                    main.x,
                    main.y + main.height,
                    main.width,
                    log_height,
                ))
            } else {
                None
            };

            LayoutAreas {
                left_drawer: left_open
                    .then(|| Rect::new(left_x, 0, left_width, main_height)),
                main,
                right_drawer: Rect::new(right_x, 0, right_width, main_height),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                overlay: None,
                log_window,
            }
        }
    }

    /// 将主区域纵向切分为消息区（上）和输入区（下）。
    ///
    /// `input_lines` 为输入框内容行数，会被限制在 1~4 行之间，
    /// 再加上 2 行用于边框，得到输入区总高度。
    pub fn split_main(main: Rect, input_lines: u16) -> (Rect, Rect) {
        let input_height = input_lines.clamp(1, 4) + 2;
        let messages_height = main.height.saturating_sub(input_height);

        let messages = Rect::new(main.x, main.y, main.width, messages_height);
        let input = Rect::new(main.x, main.y + messages_height, main.width, input_height);

        (messages, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let layout = LayoutManager::new(120, 30);
        let areas = layout.calculate();

        assert_eq!(areas.status_bar.height, 1);
        assert!(areas.left_drawer.is_none());
        // 右侧边栏始终存在
        assert_eq!(areas.right_drawer.width, (120.0_f32 * 0.28) as u16);
        assert!(areas.overlay.is_none());
        // 主区域宽度 = 120 - right_width
        let right_width = (120.0_f32 * 0.28) as u16;
        assert_eq!(areas.main.width, 120 - right_width);
    }

    #[test]
    fn test_left_drawer_expands() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.left_drawer.is_some());
        assert!(areas.overlay.is_none());
        let right_width = (120.0_f32 * 0.28) as u16;
        let left_width = (120.0_f32 * 0.22) as u16;
        assert_eq!(areas.main.width, 120 - left_width - right_width);
    }

    #[test]
    fn test_right_drawer_always_present() {
        let mut layout = LayoutManager::new(120, 30);
        // 默认状态
        let areas = layout.calculate();
        assert_eq!(areas.right_drawer.width, (120.0_f32 * 0.28) as u16);
        assert!(areas.left_drawer.is_none());

        // 左侧打开
        layout.toggle_left();
        let areas = layout.calculate();
        assert_eq!(areas.right_drawer.width, (120.0_f32 * 0.28) as u16);
        assert!(areas.left_drawer.is_some());

        // 左侧关闭
        layout.close_left();
        let areas = layout.calculate();
        assert_eq!(areas.right_drawer.width, (120.0_f32 * 0.28) as u16);
        assert!(areas.left_drawer.is_none());
    }

    #[test]
    fn test_narrow_mode_overlay() {
        let mut layout = LayoutManager::new(60, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.overlay.is_some());
        assert!(areas.left_drawer.is_none());
        // 窄屏下右侧边栏仍然常驻
        let right_width = ((60.0_f32 * 0.28) as u16).clamp(24, 40);
        assert_eq!(areas.right_drawer.width, right_width);
        // 主区域宽度 = 60 - right_width（右侧边栏始终存在）
        assert_eq!(areas.main.width, 60 - right_width);
    }

    #[test]
    fn test_main_split() {
        let main = Rect::new(0, 0, 100, 20);
        let (messages, input) = LayoutManager::split_main(main, 3);

        assert_eq!(input.height, 5);
        assert_eq!(messages.height, 15);
        assert_eq!(messages.width, 100);
        assert_eq!(input.width, 100);
    }

    #[test]
    fn test_log_window_split() {
        let mut layout = LayoutManager::new(100, 30);
        layout.log_window = true;
        let areas = layout.calculate();
        assert!(areas.log_window.is_some());
        let log = areas.log_window.unwrap();
        let main = areas.main;
        assert_eq!(main.height + log.height, 30 - 1); // minus status
        assert!(log.height > main.height);
        assert_eq!(log.y, main.y + main.height);
        assert_eq!(log.width, main.width);
    }
}
