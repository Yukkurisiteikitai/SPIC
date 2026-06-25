mod app;
mod config;
mod input;
mod model;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode};
use config::{load_settings, settings_path};
use input::{handle_key, Action};
use model::Presentation;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let pres = Presentation::demo();
    let mut app = App::new(pres);
    match load_settings(&settings_path()) {
        Ok(Some(settings)) => settings.apply_to_app(&mut app),
        Ok(None) => {}
        Err(err) => app.set_status(format!("設定読込に失敗: {}", err)),
    }

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("エラー: {}", e);
    }
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // バックグラウンドのexecから来た出力イベントを取り込む
        app.poll_exec_events();

        // モードに応じて描画関数を切り替え
        terminal.draw(|f| match app.mode {
            AppMode::Present => ui::draw_present(f, app),
            AppMode::PresentExecConfirm => ui::draw_present_exec_confirm(f, app),
            AppMode::ExecConfirm => ui::draw_exec_confirm(f, app),
            AppMode::Settings => ui::draw_settings(f, app),
            AppMode::CommandInput => ui::draw_command_input(f, app),
            _ => ui::draw(f, app),
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if let Action::Quit = handle_key(app, key) {
                    return Ok(());
                }
            }
        }
    }
}
