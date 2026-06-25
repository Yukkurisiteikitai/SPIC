/// スライドを RGBA ピクセルバッファにレンダリングする。
/// fontdue でフォントをラスタライズし、image クレートで PNG にエンコードする。
///
/// フォント / グリフ / キャンバスのキャッシュは [`crate::present::cache`] に集約されており、
/// [`render_slide_to_png_with_caches`] に注入される。
/// [`render_slide_to_png`] は後方互換のための薄いラッパ（毎回ローカルキャッシュを生成）。
use image::{ImageBuffer, Rgba};

use crate::app::App;
use crate::model::BlockKind;
use crate::present::cache::{font as cached_font, CanvasPool, GlyphCache};

// ──────────────────────────────────────────────────────────────────────────────
// カラー定義（ui.rs の BG/FG に対応）
// ──────────────────────────────────────────────────────────────────────────────
const BG: [u8; 4] = [18, 18, 18, 255];
const FG_HEADING: [u8; 4] = [224, 224, 224, 255];
const FG_TEXT: [u8; 4] = [153, 153, 153, 255];
const FG_CODE: [u8; 4] = [138, 184, 106, 255];
const FG_EXEC: [u8; 4] = [106, 176, 76, 255];
const FG_MUTED: [u8; 4] = [85, 85, 85, 255];
const FG_OUTPUT: [u8; 4] = [170, 210, 170, 255];
const FG_ACCENT: [u8; 4] = [74, 158, 255, 255];
const PANEL_BG: [u8; 4] = [24, 24, 24, 255];
const BORDER: [u8; 4] = [42, 42, 42, 255];

// ──────────────────────────────────────────────────────────────────────────────
// スライドを PNG に変換する
// ──────────────────────────────────────────────────────────────────────────────

pub struct RenderConfig {
    /// 端末の列数（セル）
    pub term_cols: u16,
    /// 端末の行数（セル）
    pub term_rows: u16,
    /// セル1つの幅[px]（端末フォント依存；デフォルト 8）
    pub cell_w: u16,
    /// セル1つの高さ[px]（端末フォント依存；デフォルト 16）
    pub cell_h: u16,
}

impl RenderConfig {
    pub fn canvas_px(&self) -> (u32, u32) {
        (
            self.term_cols as u32 * self.cell_w as u32,
            self.term_rows as u32 * self.cell_h as u32,
        )
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            term_cols: 120,
            term_rows: 36,
            cell_w: 8,
            cell_h: 16,
        }
    }
}

/// 後方互換ラッパ。内部でローカルキャッシュを生成して呼び出す。
/// パフォーマンスが必要な場合は [`render_slide_to_png_with_caches`] を使うこと。
#[allow(dead_code)]
pub fn render_slide_to_png(app: &App, cfg: &RenderConfig) -> Option<Vec<u8>> {
    let mut glyphs = GlyphCache::default();
    let mut canvas = CanvasPool::new();
    render_slide_to_png_with_caches(app, cfg, &mut glyphs, &mut canvas)
}

/// 現在のスライドを PNG バイト列に変換する（キャッシュ注入版）。
/// フォントが見つからない場合は `None` を返す（Ratatui フォールバック）。
pub fn render_slide_to_png_with_caches(
    app: &App,
    cfg: &RenderConfig,
    glyphs: &mut GlyphCache,
    canvas: &mut CanvasPool,
) -> Option<Vec<u8>> {
    let font = cached_font()?;

    let font_size_px = app.presentation.font_size as f32;
    let body_px = (font_size_px * 1.2).max(12.0);
    let heading_px = body_px * 2.0;

    let (canvas_w, canvas_h) = cfg.canvas_px();
    let buf = canvas.take(canvas_w, canvas_h, BG);
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(canvas_w, canvas_h, buf)?;

    let slide = app.current_slide();

    let margin_x = canvas_w as i32 / 10;
    let mut cursor_y: i32 = canvas_h as i32 / 12;
    let line_gap_body = (body_px * 0.4) as i32;
    let line_gap_heading = (heading_px * 0.3) as i32;
    let block_gap = (body_px * 1.0) as i32;

    let content_w = canvas_w as i32 - margin_x * 2;

    for block in &slide.blocks {
        if cursor_y > canvas_h as i32 {
            break;
        }
        match &block.kind {
            BlockKind::Heading { level } => {
                let px = if *level == 1 { heading_px } else { heading_px * 0.75 };
                let color = FG_HEADING;
                let bold = *level == 1;
                cursor_y += render_text_block(
                    &mut img, font, glyphs, &block.content, margin_x, cursor_y, content_w, px,
                    color, bold,
                );
                if *level == 1 {
                    let underline_y = cursor_y.min(canvas_h as i32 - 1);
                    draw_hline(&mut img, margin_x, underline_y, content_w, BORDER);
                    cursor_y += 4;
                }
                cursor_y += line_gap_heading;
            }
            BlockKind::Text => {
                cursor_y += render_text_block(
                    &mut img,
                    font,
                    glyphs,
                    &block.content,
                    margin_x + 20,
                    cursor_y,
                    content_w - 20,
                    body_px,
                    FG_TEXT,
                    false,
                );
                cursor_y += line_gap_body;
            }
            BlockKind::Code { .. } => {
                cursor_y += render_panel(
                    &mut img,
                    font,
                    glyphs,
                    &block.content,
                    margin_x,
                    cursor_y,
                    content_w,
                    body_px * 0.9,
                    FG_CODE,
                    PANEL_BG,
                    BORDER,
                );
                cursor_y += line_gap_body;
            }
            BlockKind::Exec { signature, .. } => {
                let border_col = if signature.is_some() {
                    FG_EXEC
                } else {
                    [255, 107, 107, 255]
                };
                let content = format!("$ {}", block.content);
                cursor_y += render_panel(
                    &mut img,
                    font,
                    glyphs,
                    &content,
                    margin_x,
                    cursor_y,
                    content_w,
                    body_px * 0.9,
                    FG_CODE,
                    PANEL_BG,
                    border_col,
                );
                cursor_y += line_gap_body;
            }
            BlockKind::OutputPlaceholder => {
                let text = if block.content.is_empty() {
                    "← output will appear here".to_string()
                } else {
                    block.content.clone()
                };
                cursor_y += render_panel(
                    &mut img,
                    font,
                    glyphs,
                    &text,
                    margin_x,
                    cursor_y,
                    content_w,
                    body_px * 0.85,
                    FG_OUTPUT,
                    PANEL_BG,
                    BORDER,
                );
                cursor_y += line_gap_body;
            }
            BlockKind::Separator => {
                draw_hline(&mut img, margin_x, cursor_y, content_w, BORDER);
                cursor_y += block_gap;
            }
        }
        cursor_y += block_gap / 2;
    }

    // フッター
    let footer = format!(
        "{}/{}  font {}px",
        app.current_slide + 1,
        app.slide_count(),
        app.presentation.font_size
    );
    render_text_block(
        &mut img,
        font,
        glyphs,
        &footer,
        margin_x,
        canvas_h as i32 - body_px as i32 - 8,
        content_w,
        body_px * 0.8,
        FG_MUTED,
        false,
    );

    let hint = "h/l スライド  +/- font  j/k ブロック  Esc 終了";
    render_text_block(
        &mut img,
        font,
        glyphs,
        hint,
        margin_x,
        canvas_h as i32 - body_px as i32 * 2 - 8,
        content_w,
        body_px * 0.75,
        FG_ACCENT,
        false,
    );

    // PNG エンコード
    let mut png_buf: Vec<u8> = Vec::new();
    {
        use image::ImageEncoder;
        image::codecs::png::PngEncoder::new(&mut png_buf)
            .write_image(img.as_raw(), canvas_w, canvas_h, image::ColorType::Rgba8)
            .ok()?;
    }

    // 使用済み RGBA バッファをプールに返却
    canvas.return_buffer(img.into_raw());

    Some(png_buf)
}

// ──────────────────────────────────────────────────────────────────────────────
// ヘルパー
// ──────────────────────────────────────────────────────────────────────────────

fn render_text_block(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    font: &fontdue::Font,
    glyphs: &mut GlyphCache,
    text: &str,
    x: i32,
    y: i32,
    max_w: i32,
    font_px: f32,
    color: [u8; 4],
    _bold: bool,
) -> i32 {
    let line_h = (font_px * 1.4) as i32;
    let mut pen_y = y;
    let img_h = img.height() as i32;
    let img_w = img.width() as i32;

    for raw_line in text.lines() {
        let char_w = estimate_char_w(font_px);
        let max_chars = ((max_w as f32) / char_w).max(1.0) as usize;

        let wrapped = wrap_text(raw_line, max_chars);
        for line in wrapped {
            if pen_y + line_h > img_h {
                break;
            }
            draw_text_line(img, font, glyphs, &line, x, pen_y, font_px, color, img_w, img_h);
            pen_y += line_h;
        }
    }
    (pen_y - y).max(line_h)
}

fn render_panel(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    font: &fontdue::Font,
    glyphs: &mut GlyphCache,
    text: &str,
    x: i32,
    y: i32,
    max_w: i32,
    font_px: f32,
    fg: [u8; 4],
    bg: [u8; 4],
    border: [u8; 4],
) -> i32 {
    let pad = (font_px * 0.5) as i32;
    let line_h = (font_px * 1.35) as i32;
    let char_w = estimate_char_w(font_px);
    let max_chars = (((max_w - pad * 2) as f32) / char_w).max(1.0) as usize;
    let img_h = img.height() as i32;
    let img_w = img.width() as i32;

    let mut all_lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        all_lines.extend(wrap_text(raw, max_chars));
    }
    if all_lines.is_empty() {
        all_lines.push(String::new());
    }
    let total_h = all_lines.len() as i32 * line_h + pad * 2;

    fill_rect(img, x, y, max_w, total_h, bg, img_w, img_h);
    draw_rect_border(img, x, y, max_w, total_h, border, img_w, img_h);

    let mut pen_y = y + pad;
    for line in &all_lines {
        draw_text_line(img, font, glyphs, line, x + pad, pen_y, font_px, fg, img_w, img_h);
        pen_y += line_h;
    }
    total_h
}

#[allow(clippy::too_many_arguments)]
fn draw_text_line(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    font: &fontdue::Font,
    glyphs: &mut GlyphCache,
    text: &str,
    x: i32,
    pen_y: i32,
    font_px: f32,
    color: [u8; 4],
    img_w: i32,
    img_h: i32,
) {
    let baseline_y = pen_y + (font_px * 0.78) as i32;
    let mut pen_x = x as f32;

    for ch in text.chars() {
        let glyph = glyphs.rasterize(font, ch, font_px);
        let metrics = &glyph.metrics;
        let bitmap = &glyph.bitmap;
        let glyph_top = baseline_y - (metrics.ymin + metrics.height as i32);
        for row in 0..metrics.height {
            let py = glyph_top + row as i32;
            if py < 0 || py >= img_h {
                continue;
            }
            for col in 0..metrics.width {
                let px_val = bitmap[row * metrics.width + col];
                if px_val == 0 {
                    continue;
                }
                let px = pen_x as i32 + metrics.xmin + col as i32;
                if px < 0 || px >= img_w {
                    continue;
                }
                let alpha = px_val as f32 / 255.0;
                let bg_px = img.get_pixel(px as u32, py as u32).0;
                let blended = [
                    blend(bg_px[0], color[0], alpha),
                    blend(bg_px[1], color[1], alpha),
                    blend(bg_px[2], color[2], alpha),
                    255,
                ];
                img.put_pixel(px as u32, py as u32, Rgba(blended));
            }
        }
        pen_x += metrics.advance_width;
    }
}

fn blend(bg: u8, fg: u8, alpha: f32) -> u8 {
    (bg as f32 * (1.0 - alpha) + fg as f32 * alpha).round() as u8
}

fn draw_hline(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    color: [u8; 4],
) {
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;
    if y < 0 || y >= img_h {
        return;
    }
    for dx in 0..w {
        let px = x + dx;
        if px >= 0 && px < img_w {
            img.put_pixel(px as u32, y as u32, Rgba(color));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: [u8; 4],
    img_w: i32,
    img_h: i32,
) {
    for dy in 0..h {
        let py = y + dy;
        if py < 0 || py >= img_h {
            continue;
        }
        for dx in 0..w {
            let px = x + dx;
            if px >= 0 && px < img_w {
                img.put_pixel(px as u32, py as u32, Rgba(color));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rect_border(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: [u8; 4],
    img_w: i32,
    img_h: i32,
) {
    for dx in 0..w {
        let px = x + dx;
        if px >= 0 && px < img_w {
            if y >= 0 && y < img_h {
                img.put_pixel(px as u32, y as u32, Rgba(color));
            }
            let by = y + h - 1;
            if by >= 0 && by < img_h {
                img.put_pixel(px as u32, by as u32, Rgba(color));
            }
        }
    }
    for dy in 0..h {
        let py = y + dy;
        if py >= 0 && py < img_h {
            if x >= 0 && x < img_w {
                img.put_pixel(x as u32, py as u32, Rgba(color));
            }
            let rx = x + w - 1;
            if rx >= 0 && rx < img_w {
                img.put_pixel(rx as u32, py as u32, Rgba(color));
            }
        }
    }
}

fn estimate_char_w(font_px: f32) -> f32 {
    font_px * 0.6
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut count = 0;
    for ch in text.chars() {
        let w = if ch.is_ascii() { 1 } else { 2 };
        if count + w > max_chars && !current.is_empty() {
            lines.push(current.clone());
            current.clear();
            count = 0;
        }
        current.push(ch);
        count += w;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_works() {
        let lines = wrap_text("Hello World", 6);
        assert!(lines.len() >= 2, "expected wrap, got {:?}", lines);
    }

    #[test]
    fn render_slide_to_png_generates_bytes() {
        use crate::app::{App, AppMode};
        use crate::model::Presentation;
        let pres = Presentation::demo();
        let mut app = App::new(pres);
        app.presentation.font_size = 20;
        app.mode = AppMode::Present;
        let cfg = RenderConfig::default();
        let result = render_slide_to_png(&app, &cfg);
        assert!(
            result.is_some(),
            "render_slide_to_png returned None (font not found?)"
        );
        let png = result.unwrap();
        assert!(png.len() > 1000, "PNG too small: {} bytes", png.len());
        assert_eq!(&png[..4], b"\x89PNG", "not a valid PNG");
        eprintln!("PNG generated: {} bytes", png.len());
    }

    #[test]
    fn render_with_cache_matches_without_cache() {
        use crate::app::{App, AppMode};
        use crate::model::Presentation;
        let pres = Presentation::demo();
        let mut app = App::new(pres);
        app.presentation.font_size = 18;
        app.mode = AppMode::Present;
        let cfg = RenderConfig::default();

        let png_a = render_slide_to_png(&app, &cfg);
        let mut glyphs = GlyphCache::default();
        let mut canvas = CanvasPool::new();
        let png_b = render_slide_to_png_with_caches(&app, &cfg, &mut glyphs, &mut canvas);
        // 2 度目（キャッシュヒット）でも同じバイト列が出る
        let png_c = render_slide_to_png_with_caches(&app, &cfg, &mut glyphs, &mut canvas);
        assert_eq!(png_a, png_b);
        assert_eq!(png_b, png_c);
    }
}

#[cfg(test)]
mod write_png_test {
    use super::*;
    use crate::app::{App, AppMode};
    use crate::model::Presentation;

    #[test]
    #[ignore]
    fn write_png_to_tmp() {
        let pres = Presentation::demo();
        let mut app = App::new(pres);
        app.go_to_slide(1);
        app.presentation.font_size = 27;
        app.mode = AppMode::Present;
        let cfg = RenderConfig {
            term_cols: 120,
            term_rows: 36,
            cell_w: 12,
            cell_h: 24,
        };
        let png = render_slide_to_png(&app, &cfg).expect("render failed");
        std::fs::write("/tmp/slidecli_preview.png", &png).unwrap();
        eprintln!(
            "Written to /tmp/slidecli_preview.png ({} bytes)",
            png.len()
        );
    }
}
