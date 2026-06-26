mod app;
mod input;
mod markdown;
mod model;
mod ui;

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode};
use input::{handle_key, Action};
use model::Presentation;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (pres, current_file, status_message) = load_initial_presentation()?;
    let mut app = if let Some(path) = current_file {
        App::with_file(pres, Some(path))
    } else {
        App::new(pres)
    };
    if let Some(message) = status_message {
        app.set_status(message);
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

fn load_initial_presentation() -> Result<(Presentation, Option<PathBuf>, Option<String>)> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => Ok((Presentation::demo(), None, Some("デモを開きました".to_string()))),
        [path] => load_from_path(PathBuf::from(path)),
        [command, path] if command == "edit" => load_from_path(PathBuf::from(path)),
        _ => Err(anyhow!(
            "使い方: slidecli [presentation.md] または slidecli edit presentation.md"
        )),
    }
}

fn load_from_path(path: PathBuf) -> Result<(Presentation, Option<PathBuf>, Option<String>)> {
    if path.exists() {
        let source = fs::read_to_string(&path)?;
        let presentation =
            markdown::deserialize(&source).map_err(|err| anyhow!("Markdown parse error: {}", err))?;
        Ok((
            presentation,
            Some(path.clone()),
            Some(format!("開きました: {}", path.display())),
        ))
    } else {
        Ok((
            Presentation::blank(),
            Some(path.clone()),
            Some(format!("新規ファイルを作成します: {}", path.display())),
        ))
    }
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
