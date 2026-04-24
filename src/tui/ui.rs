use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::app::{Message, MessageRole, TuiApp};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 状态栏
            Constraint::Min(3),    // 消息区
            Constraint::Length(3), // 输入框
        ])
        .split(frame.area());

    draw_status_bar(frame, app, chunks[0]);
    draw_messages(frame, app, chunks[1]);
    draw_input_box(frame, app, chunks[2]);

    if app.show_dropdown {
        draw_command_dropdown(frame, app);
    }
}

fn draw_status_bar(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let spinner = if app.waiting.load(std::sync::atomic::Ordering::Relaxed) {
        SPINNER_FRAMES[app.spinner_frame]
    } else {
        " "
    };

    let text = format!("FiCode {} | model: {}", spinner, app.current_model);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_messages(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let messages: Vec<Line> = app
        .messages
        .iter()
        .map(|msg| {
            let (prefix, color) = match msg.role {
                MessageRole::User => ("> ", Color::Green),
                MessageRole::Assistant => ("🤖 ", Color::Blue),
                MessageRole::System => ("ℹ️ ", Color::Yellow),
                MessageRole::Error => ("❌ ", Color::Red),
            };
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::raw(&msg.content),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(messages))
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .wrap(Wrap { trim: true })
        .scroll((app.scroll_offset as u16, 0));

    frame.render_widget(paragraph, area);
}

fn draw_input_box(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let input_text = format!("> {}", app.input);
    let paragraph = Paragraph::new(input_text)
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_command_dropdown(frame: &mut Frame, app: &TuiApp) {
    let commands = vec![
        ("/model", "切换模型"),
        ("/init", "生成 AGENTS.md"),
        ("/help", "显示帮助"),
    ];

    let items: Vec<Line> = commands
        .iter()
        .enumerate()
        .map(|(i, (cmd, desc))| {
            let style = if i == app.dropdown_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
            };
            Line::styled(format!("{} — {}", cmd, desc), style)
        })
        .collect();

    let height = items.len() as u16 + 2;
    let width = 30;
    let x = (frame.area().width.saturating_sub(width)) / 2;
    let y = frame.area().height.saturating_sub(6 + height);

    let area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, area);

    let paragraph = Paragraph::new(Text::from(items))
        .block(Block::default().borders(Borders::ALL).title("Commands"));
    frame.render_widget(paragraph, area);
}
