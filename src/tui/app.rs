use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::server::sse::SseEvent;
use crate::tui::components::{
    chat::Chat, header::Header, input::Input, left_drawer::LeftDrawer, right_drawer::RightDrawer,
    status_bar::StatusBar, Component,
};
use crate::tui::event::{AppEvent, FocusArea};
use crate::tui::layout::{LayoutManager, PanelState};
use crate::tui::theme::Theme;

use super::client::TuiClient;

pub struct TuiApp {
    layout: LayoutManager,
    theme: Arc<Theme>,
    themes: Vec<Arc<Theme>>,
    theme_index: usize,

    header: Header,
    left_drawer: LeftDrawer,
    right_drawer: RightDrawer,
    chat: Chat,
    input: Input,
    status_bar: StatusBar,

    focus: FocusArea,
    is_generating: bool,
    should_quit: bool,

    client: TuiClient,
    event_tx: mpsc::Sender<AppEvent>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl TuiApp {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let themes = vec![
            Arc::new(Theme::deep_ocean()),
            Arc::new(Theme::github_dark()),
        ];

        Self {
            layout: LayoutManager::new(80, 24),
            theme: themes[0].clone(),
            themes,
            theme_index: 0,
            header: Header::new(),
            left_drawer: LeftDrawer::new(),
            right_drawer: RightDrawer::new(),
            chat: Chat::new(),
            input: Input::new(),
            status_bar: StatusBar::new(),
            focus: FocusArea::Main,
            is_generating: false,
            should_quit: false,
            client: TuiClient::new(),
            event_tx,
            event_rx,
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        if let Ok(model) = self.client.get_status().await {
            self.header.set_current_model(model);
        }

        let mut interval = tokio::time::interval(Duration::from_millis(80));

        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {
                _ = interval.tick() => {
                    self.handle_app_event(AppEvent::Tick).await;
                }
                Some(event) = self.event_rx.recv() => {
                    self.handle_app_event(event).await;
                }
                result = Self::read_crossterm_event() => {
                    if let Ok(event) = result {
                        self.route_event(event).await;
                    }
                }
            }
        }

        Ok(())
    }

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

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let areas = self.layout.calculate();
        let input_lines = self.input.visible_lines();
        let (messages_area, input_area) = LayoutManager::split_main(areas.main, input_lines);

        self.header.draw(frame, areas.header, &self.theme);
        self.chat.draw(frame, messages_area, &self.theme);
        self.input.draw(frame, input_area, &self.theme);
        self.status_bar.draw(frame, areas.status_bar, &self.theme);

        if let Some(overlay_area) = areas.overlay {
            let dim = ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(self.theme.bg_overlay));
            frame.render_widget(dim, areas.main);

            match self.layout.panel {
                PanelState::LeftDrawer => {
                    self.left_drawer.draw(frame, overlay_area, &self.theme);
                }
                PanelState::RightDrawer => {
                    self.right_drawer.draw(frame, overlay_area, &self.theme);
                }
                _ => {}
            }
        } else {
            if let Some(area) = areas.left_drawer {
                self.left_drawer.draw(frame, area, &self.theme);
            }
            if let Some(area) = areas.right_drawer {
                self.right_drawer.draw(frame, area, &self.theme);
            }
        }
    }

    fn next_theme(&mut self) {
        self.theme_index = (self.theme_index + 1) % self.themes.len();
        self.theme = self.themes[self.theme_index].clone();
    }

    async fn route_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    if self.is_generating {
                        self.handle_app_event(AppEvent::StopGeneration).await;
                    } else {
                        self.should_quit = true;
                    }
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                    self.layout.toggle_left();
                    self.focus = FocusArea::LeftDrawer;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('h')) => {
                    self.layout.toggle_right();
                    self.focus = FocusArea::RightDrawer;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('m')) => {
                    self.header.toggle_model_dropdown();
                    self.focus = FocusArea::Header;
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('t')) => {
                    self.next_theme();
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    if self.layout.panel != PanelState::None {
                        self.layout.close_drawers();
                    } else {
                        self.focus = FocusArea::Main;
                    }
                    return;
                }
                _ => {}
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

    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                self.chat.on_tick();
                self.header.on_tick();
            }
            AppEvent::Resize(w, h) => {
                self.layout.resize(w, h);
            }
            AppEvent::SubmitMessage(ref msg) => {
                self.is_generating = true;
                self.chat.add_user_message(msg);
                self.start_chat_stream(msg.clone()).await;
            }
            AppEvent::SseEvent(ref sse_event) => {
                self.chat.handle_sse_event(sse_event);
                if let SseEvent::Done { session_id } = sse_event {
                    self.header.set_session_id(session_id.clone());
                }
            }
            AppEvent::ChatComplete => {
                self.is_generating = false;
            }
            AppEvent::StopGeneration => {
                self.is_generating = false;
            }
            _ => {}
        }

        self.header.update(&event);
        self.chat.update(&event);
        self.input.update(&event);
        self.left_drawer.update(&event);
        self.right_drawer.update(&event);
        self.status_bar.update(&event);
    }

    async fn start_chat_stream(&self, message: String) {
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        let session_id = self.header.session_id();

        tokio::spawn(async move {
            let (sse_tx, mut sse_rx) = mpsc::channel(100);

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
}
