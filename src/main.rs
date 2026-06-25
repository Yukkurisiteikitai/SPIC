mod app;
mod config;
mod input;
mod kgp;
mod model;
mod render;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute, queue,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode};
use config::{load_settings, settings_path};
use input::{handle_key, Action};
use model::Presentation;
use render::{render_slide_to_png, RenderConfig};

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
    // スライド変更を検出して画像キャッシュを無効化
    let mut last_slide = usize::MAX;
    let mut last_font_size = u8::MAX;
    let mut kgp_cfg = RenderConfig::default();

    loop {
        app.poll_exec_events();

        let in_present = matches!(app.mode, AppMode::Present | AppMode::PresentExecConfirm);

        if use_kgp && in_present {
            // スライドか font_size が変わったときだけ再レンダリング
            let slide_changed = app.current_slide != last_slide;
            let font_changed = app.presentation.font_size != last_font_size;

            if slide_changed || font_changed {
                // 端末サイズを取得してキャンバスを更新
                let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 36));
                kgp_cfg.term_cols = cols;
                kgp_cfg.term_rows = rows.saturating_sub(1); // フッター1行ぶん除く
                // セルサイズは環境変数 SLIDECLI_CELL_W/H で上書き可能
                kgp_cfg.cell_w = env_u16("SLIDECLI_CELL_W", 8);
                kgp_cfg.cell_h = env_u16("SLIDECLI_CELL_H", 16);

                // Ratatui の描画を一時止めて KGP を直接送信
                // crossterm cursor を左上に移動してから送る
                {
                    use std::io::Write;
                    let mut out = io::stdout();
                    // 画面クリア
                    queue!(out, crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
                    queue!(out, cursor::MoveTo(0, 0))?;
                    out.flush()?;
                }

                if let Some(png) = render_slide_to_png(app, &kgp_cfg) {
                    kgp::display_png(&png, 0, 0, cols, kgp_cfg.term_rows)?;
                } else {
                    // フォント未対応 → Ratatui フォールバック
                    draw_with_ratatui(terminal, app)?;
                }

                last_slide = app.current_slide;
                last_font_size = app.presentation.font_size;
            }

            // プレゼン中 exec 実行確認ダイアログは Ratatui で重ね描き
            if matches!(app.mode, AppMode::PresentExecConfirm) {
                terminal.draw(|f| ui::draw_present_exec_confirm(f, app))?;
            }
        } else {
            // 非プレゼントモード or KGP 非対応 → 通常 Ratatui
            if use_kgp && !in_present && last_slide != usize::MAX {
                // プレゼントから抜けた直後: KGP 画像をクリア
                kgp::clear_image()?;
                last_slide = usize::MAX;
                last_font_size = u8::MAX;
                // Ratatui に戻る前に画面を再描画
                terminal.clear()?;
            }
            draw_with_ratatui(terminal, app)?;
        }

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if let Action::Quit = handle_key(app, key) {
                    return Ok(());
                }
                // プレゼント中にスライドが変わったら強制再描画
                if in_present && use_kgp {
                    if app.current_slide != last_slide || app.presentation.font_size != last_font_size {
                        last_slide = usize::MAX; // 無効化してループ頭で再レンダリング
                    }
                }
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

fn env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
