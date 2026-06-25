//! プレゼン中 1 フレームのライフサイクル統合エントリ。
//!
//! `main::run_app` から呼ばれ、以下を担当する:
//! - 端末サイズの取得と `RenderConfig` 更新
//! - `FrameFingerprint` の計算
//! - キャッシュヒット時の描画スキップ
//! - キャッシュミス時の PNG レンダリング・base64 エンコード・KGP 送信
//! - `PresentState` の更新（dirty flag のクリア、最後の描画時刻）

use std::io::{self, Write};
use std::rc::Rc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use base64::Engine;
use crossterm::{cursor, queue};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::kgp;
use crate::present::cache::{CachedFrame, CanvasPool, FrameCache, GlyphCache};
use crate::present::fingerprint::FrameFingerprint;
use crate::render::{render_slide_to_png_with_caches, RenderConfig};

pub struct PresentCaches {
    pub glyphs: GlyphCache,
    pub canvas: CanvasPool,
    pub frames: FrameCache,
}

impl Default for PresentCaches {
    fn default() -> Self {
        Self::new()
    }
}

impl PresentCaches {
    pub fn new() -> Self {
        Self {
            glyphs: GlyphCache::default(),
            canvas: CanvasPool::new(),
            frames: FrameCache::new(),
        }
    }
}

/// プレゼン中の 1 フレーム処理。
///
/// - `app.present.needs_redraw` または fingerprint 変化があれば再描画
/// - キャッシュヒット時は KGP 送信もスキップして即 return（CPU/IO ゼロ）
/// - `SLIDECLI_FORCE_REDRAW=1` で fingerprint を無視して毎フレーム描画（escape hatch）
pub fn tick_present_kgp(
    _terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    caches: &mut PresentCaches,
    kgp_cfg: &mut RenderConfig,
) -> Result<()> {
    // 端末サイズと RenderConfig を毎回更新（リサイズ追従）
    let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 36));
    let term_rows = rows.saturating_sub(1);
    kgp_cfg.term_cols = cols;
    kgp_cfg.term_rows = term_rows;
    kgp_cfg.cell_w = env_u16("SLIDECLI_CELL_W", 8);
    kgp_cfg.cell_h = env_u16("SLIDECLI_CELL_H", 16);
    let canvas_size = (
        kgp_cfg.term_cols as u32 * kgp_cfg.cell_w as u32,
        kgp_cfg.term_rows as u32 * kgp_cfg.cell_h as u32,
    );

    // exec 実行中は経過時間表示が秒単位で変わるため、1 秒経過で dirty flag
    if app.running_exec.is_some() {
        let stale = app
            .present
            .last_render_at
            .map(|t| t.elapsed() >= Duration::from_secs(1))
            .unwrap_or(true);
        if stale {
            app.present.needs_redraw = true;
        }
    }

    let fp = FrameFingerprint::compute(app, canvas_size);
    let force = std::env::var("SLIDECLI_FORCE_REDRAW").is_ok();

    // 既に表示済みかつ fingerprint 一致なら完全スキップ
    if !force && !app.present.needs_redraw {
        if let Some(prev) = &app.present.last_displayed_fingerprint {
            if prev == &fp {
                return Ok(());
            }
        }
    }

    // キャッシュヒット（同じ fingerprint の PNG/base64 を保持済み）
    let frame = if let Some(cached) = caches.frames.lookup(&fp) {
        cached.clone()
    } else {
        let png = render_slide_to_png_with_caches(app, kgp_cfg, &mut caches.glyphs, &mut caches.canvas)
            .ok_or_else(|| anyhow!("render_slide_to_png returned None (font not found?)"))?;
        let png = Rc::new(png);
        let base64 = Rc::new(base64::engine::general_purpose::STANDARD.encode(png.as_ref()));
        let frame = CachedFrame {
            fingerprint: fp.clone(),
            png,
            base64,
        };
        caches.frames.store(frame.clone());
        frame
    };

    // カーソルを左上に移動してから画像送信
    // SLIDECLI_LEGACY_CLEAR=1 ならフレームごとに Clear(All) を実行（旧挙動）
    let legacy_clear = std::env::var("SLIDECLI_LEGACY_CLEAR").is_ok();
    {
        let mut out = io::stdout();
        if legacy_clear {
            queue!(
                out,
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            )?;
        }
        queue!(out, cursor::MoveTo(0, 0))?;
        out.flush()?;
    }

    kgp::display_png_encoded(&frame.base64, 0, 0, kgp_cfg.term_cols, kgp_cfg.term_rows)?;
    app.present.mark_rendered(fp, (cols, rows));

    Ok(())
}

fn env_u16(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
