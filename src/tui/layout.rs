use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    None,
    LeftDrawer,
    RightDrawer,
}

pub struct LayoutManager {
    pub terminal_size: (u16, u16),
    pub panel: PanelState,
    pub narrow_mode: bool,
}

#[derive(Debug)]
pub struct LayoutAreas {
    pub header: Rect,
    pub left_drawer: Option<Rect>,
    pub main: Rect,
    pub right_drawer: Option<Rect>,
    pub status_bar: Rect,
    pub overlay: Option<Rect>,
}

impl LayoutManager {
    pub fn new(width: u16, height: u16) -> Self {
        let narrow_mode = width < 80;
        Self {
            terminal_size: (width, height),
            panel: PanelState::None,
            narrow_mode,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
        self.narrow_mode = width < 80;
    }

    pub fn toggle_left(&mut self) {
        self.panel = match self.panel {
            PanelState::LeftDrawer => PanelState::None,
            _ => PanelState::LeftDrawer,
        };
    }

    pub fn toggle_right(&mut self) {
        self.panel = match self.panel {
            PanelState::RightDrawer => PanelState::None,
            _ => PanelState::RightDrawer,
        };
    }

    pub fn close_drawers(&mut self) {
        self.panel = PanelState::None;
    }

    pub fn calculate(&self) -> LayoutAreas {
        let (width, height) = self.terminal_size;
        let header_height = 3u16;
        let status_height = 1u16;
        let main_height = height.saturating_sub(header_height + status_height);

        if self.narrow_mode && self.panel != PanelState::None {
            let overlay_width = (width as f32 * 0.75).max(30.0).min(width as f32) as u16;
            let overlay_x = match self.panel {
                PanelState::LeftDrawer => 0,
                PanelState::RightDrawer => width.saturating_sub(overlay_width),
                PanelState::None => 0,
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                main: Rect::new(0, header_height, width, main_height),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                left_drawer: None,
                right_drawer: None,
                overlay: Some(Rect::new(
                    overlay_x,
                    header_height,
                    overlay_width,
                    main_height,
                )),
            }
        } else {
            let drawer_width = ((width as f32 * 0.28) as u16).clamp(24, 40);
            let main_width = match self.panel {
                PanelState::None => width,
                _ => width.saturating_sub(drawer_width),
            };

            let (left_x, main_x, right_x) = match self.panel {
                PanelState::LeftDrawer => (0, drawer_width, width),
                PanelState::RightDrawer => (0, 0, main_width),
                PanelState::None => (0, 0, width),
            };

            LayoutAreas {
                header: Rect::new(0, 0, width, header_height),
                left_drawer: (self.panel == PanelState::LeftDrawer)
                    .then(|| Rect::new(left_x, header_height, drawer_width, main_height)),
                main: Rect::new(main_x, header_height, main_width, main_height),
                right_drawer: (self.panel == PanelState::RightDrawer)
                    .then(|| Rect::new(right_x, header_height, drawer_width, main_height)),
                status_bar: Rect::new(0, height - status_height, width, status_height),
                overlay: None,
            }
        }
    }

    pub fn split_main(main: Rect, input_lines: u16) -> (Rect, Rect) {
        let input_height = input_lines.clamp(1, 5) + 2;
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

        assert_eq!(areas.header.height, 3);
        assert_eq!(areas.status_bar.height, 1);
        assert!(areas.left_drawer.is_none());
        assert!(areas.right_drawer.is_none());
        assert!(areas.overlay.is_none());
        assert_eq!(areas.main.width, 120);
    }

    #[test]
    fn test_left_drawer_expands() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.left_drawer.is_some());
        assert!(areas.right_drawer.is_none());
        assert!(areas.overlay.is_none());
        assert!(areas.main.width < 120);
    }

    #[test]
    fn test_drawer_mutual_exclusion() {
        let mut layout = LayoutManager::new(120, 30);
        layout.toggle_left();
        layout.toggle_right();

        assert_eq!(layout.panel, PanelState::RightDrawer);
        let areas = layout.calculate();
        assert!(areas.left_drawer.is_none());
        assert!(areas.right_drawer.is_some());
    }

    #[test]
    fn test_narrow_mode_overlay() {
        let mut layout = LayoutManager::new(60, 30);
        layout.toggle_left();
        let areas = layout.calculate();

        assert!(areas.overlay.is_some());
        assert!(areas.left_drawer.is_none());
        assert_eq!(areas.main.width, 60);
    }

    #[test]
    fn test_main_split() {
        let main = Rect::new(0, 3, 100, 20);
        let (messages, input) = LayoutManager::split_main(main, 3);

        assert_eq!(input.height, 5);
        assert_eq!(messages.height, 15);
        assert_eq!(messages.width, 100);
        assert_eq!(input.width, 100);
    }
}
