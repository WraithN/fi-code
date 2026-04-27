pub mod app;
pub mod client;
pub mod components;
pub mod event;
pub mod layout;
pub mod theme;

use app::TuiApp;

pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    ratatui::restore();
    result
}
