use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::server::sse::SseEvent;

use super::client::TuiClient;
use super::ui;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Debug)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Error,
}

pub struct TuiApp {
    pub input: String,
    pub messages: Vec<Message>,
    pub current_model: String,
    pub waiting: Arc<AtomicBool>,
    pub show_dropdown: bool,
    pub dropdown_selected: usize,
    pub session_id: Option<String>,
    pub spinner_frame: usize,
    pub scroll_offset: usize,
    pub client: TuiClient,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub event_rx: mpsc::Receiver<AppEvent>,
}

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    SseEvent(SseEvent),
    ChatComplete(Result<String, String>),
    ExecuteComplete(Result<String, String>),
}

impl TuiApp {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        Self {
            input: String::new(),
            messages: Vec::new(),
            current_model: "unknown".to_string(),
            waiting: Arc::new(AtomicBool::new(false)),
            show_dropdown: false,
            dropdown_selected: 0,
            session_id: None,
            spinner_frame: 0,
            scroll_offset: 0,
            client: TuiClient::new(),
            event_tx,
            event_rx,
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let tick_interval = std::time::Duration::from_millis(80);
        let mut last_tick = tokio::time::Instant::now();

        loop {
            terminal.draw(|frame| ui::draw(frame, self))?;

            let timeout = tick_interval.saturating_sub(
                std::time::Duration::from_millis(last_tick.elapsed().as_millis() as u64)
            );

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            break;
                        }
                        self.handle_key(key.code).await;
                    }
                }
            }

            if last_tick.elapsed() >= tick_interval {
                self.on_tick();
                last_tick = tokio::time::Instant::now();
            }

            while let Ok(event) = self.event_rx.try_recv() {
                self.handle_app_event(event).await;
            }
        }

        Ok(())
    }

    async fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter => self.on_submit().await,
            KeyCode::Backspace => { self.input.pop(); self.update_dropdown(); }
            KeyCode::Up => self.on_up(),
            KeyCode::Down => self.on_down(),
            KeyCode::Esc => self.show_dropdown = false,
            KeyCode::Char(c) => { self.input.push(c); self.update_dropdown(); }
            _ => {}
        }
    }

    fn update_dropdown(&mut self) {
        if self.input.starts_with('/') {
            self.show_dropdown = true;
            self.dropdown_selected = 0;
        } else {
            self.show_dropdown = false;
        }
    }

    fn on_up(&mut self) {
        if self.show_dropdown && self.dropdown_selected > 0 {
            self.dropdown_selected -= 1;
        }
    }

    fn on_down(&mut self) {
        if self.show_dropdown {
            if self.dropdown_selected < 2 {
                self.dropdown_selected += 1;
            }
        }
    }

    async fn on_submit(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        self.messages.push(Message {
            role: MessageRole::User,
            content: input.clone(),
        });
        self.input.clear();
        self.show_dropdown = false;
        self.scroll_offset = 0;

        if input.starts_with('/') {
            self.execute_command(input).await;
        } else {
            self.send_chat(input).await;
        }
    }

    async fn execute_command(&mut self, command: String) {
        self.waiting.store(true, Ordering::Relaxed);
        let client = self.client.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            let result = client.execute(&command).await;
            let _ = tx.send(AppEvent::ExecuteComplete(
                result.map_err(|e| e.to_string())
            )).await;
        });
    }

    async fn send_chat(&mut self, message: String) {
        self.waiting.store(true, Ordering::Relaxed);
        let client = self.client.clone();
        let session_id = self.session_id.clone();
        let tx = self.event_tx.clone();
        let sse_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let (sse_event_tx, mut sse_event_rx) = mpsc::channel(100);

            let forward_handle = tokio::spawn(async move {
                while let Some(event) = sse_event_rx.recv().await {
                    let _ = sse_tx.send(AppEvent::SseEvent(event)).await;
                }
            });

            let result = client.chat(session_id, message, sse_event_tx).await;
            let _ = forward_handle.await;

            let _ = tx.send(AppEvent::ChatComplete(
                result.map_err(|e| e.to_string())
            )).await;
        });
    }

    async fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                if self.waiting.load(Ordering::Relaxed) {
                    self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
                }
            }
            AppEvent::SseEvent(sse) => match sse {
                SseEvent::Message { content } => {
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == MessageRole::Assistant {
                            last.content.push_str(&content);
                            return;
                        }
                    }
                    self.messages.push(Message {
                        role: MessageRole::Assistant,
                        content,
                    });
                }
                SseEvent::Done { session_id } => {
                    self.session_id = Some(session_id);
                }
                SseEvent::Error { message } => {
                    self.messages.push(Message {
                        role: MessageRole::Error,
                        content: message,
                    });
                }
                _ => {}
            },
            AppEvent::ChatComplete(result) => {
                self.waiting.store(false, Ordering::Relaxed);
                if let Err(e) = result {
                    self.messages.push(Message {
                        role: MessageRole::Error,
                        content: e,
                    });
                }
            }
            AppEvent::ExecuteComplete(result) => {
                self.waiting.store(false, Ordering::Relaxed);
                match result {
                    Ok(msg) => self.messages.push(Message {
                        role: MessageRole::System,
                        content: msg,
                    }),
                    Err(e) => self.messages.push(Message {
                        role: MessageRole::Error,
                        content: e,
                    }),
                }
            }
        }
    }

    fn on_tick(&mut self) {
        if self.waiting.load(Ordering::Relaxed) {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
    }
}
