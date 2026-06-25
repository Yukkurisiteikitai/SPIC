mod app;
mod config;
mod input;
mod kgp;
mod model;
mod present;
mod render;
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
use present::runtime::{tick_present_kgp, PresentCaches};
use render::RenderConfig;

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

    let use_kgp = kgp::is_supported();
    let result = run_app(&mut terminal, &mut app, use_kgp);

    if use_kgp {
        let _ = kgp::clear_image();
    }

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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    use_kgp: bool,
) -> Result<()> {
    let mut kgp_cfg = RenderConfig::default();
    let mut caches = PresentCaches::new();
    // プレゼンから抜けた直後かどうかを追跡（KGP クリアのトリガ）
    let mut was_in_present = false;

    loop {
        app.poll_exec_events();

        let in_present = matches!(app.mode, AppMode::Present | AppMode::PresentExecConfirm);

        if use_kgp && in_present {
            // KGP パス: fingerprint が変わったときだけ再描画
            tick_present_kgp(terminal, app, &mut caches, &mut kgp_cfg)?;

            // プレゼン中 exec 実行確認ダイアログは Ratatui で重ね描き
            if matches!(app.mode, AppMode::PresentExecConfirm) {
                terminal.draw(|f| ui::draw_present_exec_confirm(f, app))?;
            }

            was_in_present = true;
        } else {
            if use_kgp && was_in_present {
                // プレゼンから抜けた直後: KGP 画像をクリアし Ratatui に戻す
                kgp::clear_image()?;
                caches.frames.clear();
                terminal.clear()?;
                was_in_present = false;
            }
            draw_with_ratatui(terminal, app)?;
        }

        // 動的 poll timeout:
        //   プレゼン中アイドル → 500ms（CPU 節約）
        //   プレゼン中 exec 実行中 → 50ms（出力反映の応答性）
        //   編集モード等 → 16ms（UI の応答性）
        // SLIDECLI_POLL_MS で上書き可能
        let default_ms: u64 = match app.mode {
            AppMode::Present | AppMode::PresentExecConfirm => {
                if app.running_exec.is_some() { 50 } else { 500 }
            }
            _ => 16,
        };
        let poll_ms = env_u64("SLIDECLI_POLL_MS", default_ms);
        let timeout = Duration::from_millis(poll_ms);

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if let Action::Quit = handle_key(app, key) {
                        return Ok(());
                    }
                    // キー入力はプレゼン画面の再描画候補に
                    app.present.needs_redraw = true;
                }
                Event::Resize(_, _) => {
                    // 端末リサイズ: キャンバスサイズが変わるので fingerprint が自動的にミスする。
                    // ここでは明示的に dirty flag と frame キャッシュをクリアして確実に再描画。
                    app.present.needs_redraw = true;
                    caches.frames.clear();
                    let _ = terminal.autoresize();
                }
                _ => {}
            }
        }
    }
}

fn draw_with_ratatui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    terminal.draw(|f| match app.mode {
        AppMode::Present => ui::draw_present(f, app),
        AppMode::PresentExecConfirm => ui::draw_present_exec_confirm(f, app),
        AppMode::ExecConfirm => ui::draw_exec_confirm(f, app),
        AppMode::Settings => ui::draw_settings(f, app),
        AppMode::CommandInput => ui::draw_command_input(f, app),
        _ => ui::draw(f, app),
    })?;
    Ok(())
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
