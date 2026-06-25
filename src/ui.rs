use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, AppMode};
use crate::model::BlockKind;

// ── カラーパレット（モックのダークテーマに合わせる）─────────────
const BG_BASE: Color      = Color::Rgb(26, 26, 26);     // #1a1a1a  メイン背景
const BG_SIDEBAR: Color   = Color::Rgb(22, 22, 22);     // #161616  サイドバー
const BG_TITLEBAR: Color  = Color::Rgb(17, 17, 17);     // #111111  タイトルバー
const BG_STATUSBAR: Color = Color::Rgb(13, 13, 13);     // #0d0d0d  ステータスバー
const BG_TOOLBAR: Color   = Color::Rgb(17, 17, 17);     // #111111  ツールバー
const BG_SLIDE: Color     = Color::Rgb(36, 36, 36);     // #242424  スライドキャンバス

const BG_BLOCK_SEL: Color = Color::Rgb(20, 30, 50);     // 選択中ブロック背景
const BG_EXEC: Color      = Color::Rgb(20, 40, 20);     // execブロック背景
const BG_EXEC_SEL: Color  = Color::Rgb(20, 50, 20);     // exec選択中

const FG_PRIMARY: Color   = Color::Rgb(224, 224, 224);  // #e0e0e0  メインテキスト
const FG_SECONDARY: Color = Color::Rgb(153, 153, 153);  // #999999  サブテキスト
const FG_MUTED: Color     = Color::Rgb(85, 85, 85);     // #555555  スライド番号等
const FG_ACCENT: Color    = Color::Rgb(74, 158, 255);   // #4a9eff  青アクセント
const FG_EXEC: Color      = Color::Rgb(106, 176, 76);   // #6ab04c  execアクセント
const FG_WARN: Color      = Color::Rgb(255, 107, 107);  // #ff6b6b  警告
const FG_CODE: Color      = Color::Rgb(138, 184, 106);  // #8ab86a  コード
const FG_ACTIVE_SLIDE: Color = Color::Rgb(204, 204, 204); // アクティブスライド

const BORDER_DIM: Color   = Color::Rgb(42, 42, 42);     // #2a2a2a  薄いボーダー
const BORDER_SEL: Color   = Color::Rgb(74, 158, 255);   // 選択中ボーダー（青）
const BORDER_EXEC: Color  = Color::Rgb(58, 90, 42);     // execボーダー（緑）
const BORDER_EXEC_SEL: Color = Color::Rgb(106, 176, 76); // exec選択中ボーダー

pub fn draw(f: &mut Frame<'_>, app: &App) {
    let area = f.size();

    // 全体背景
    f.render_widget(
        Block::default().style(Style::default().bg(BG_BASE)),
        area,
    );

    // タイトルバー(1行) / ボディ / ツールバー(1行) / ステータスバー(1行)
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // タイトルバー
            Constraint::Min(0),      // ボディ
            Constraint::Length(1),   // ツールバー
            Constraint::Length(1),   // ステータスバー
        ])
        .split(area);

    draw_titlebar(f, app, root[0]);
    draw_body(f, app, root[1]);
    draw_toolbar(f, app, root[2]);
    draw_statusbar(f, app, root[3]);

    // オーバーレイ（ブロック追加パレット等）
    if app.mode == AppMode::BlockPicker {
        draw_block_picker(f, app, area);
    }
}

// ── タイトルバー ─────────────────────────────────────────────────
fn draw_titlebar(f: &mut Frame<'_>, app: &App, area: Rect) {
    let pres = &app.presentation;
    let title = format!(
        " slidecli  —  demo.md [{}/{}]  {}",
        app.current_slide + 1,
        app.slide_count(),
        pres.font_name,
    );
    let widget = Paragraph::new(title)
        .style(Style::default().bg(BG_TITLEBAR).fg(FG_MUTED))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(widget, area);
}

// ── ボディ（サイドバー + キャンバス）────────────────────────────
fn draw_body(f: &mut Frame<'_>, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),  // サイドバー幅
            Constraint::Min(0),      // キャンバス
        ])
        .split(area);

    draw_sidebar(f, app, cols[0]);
    draw_canvas(f, app, cols[1]);
}

// ── サイドバー（スライドサムネイル一覧）────────────────────────
fn draw_sidebar(f: &mut Frame<'_>, app: &App, area: Rect) {
    // 背景
    f.render_widget(
        Block::default().style(Style::default().bg(BG_SIDEBAR)),
        area,
    );

    // 右ボーダーライン
    let border = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(BORDER_DIM));
    f.render_widget(border, area);

    let inner = Rect {
        x: area.x,
        y: area.y,
        width: area.width - 1,
        height: area.height,
    };

    let mut items: Vec<ListItem> = Vec::new();

    for (i, slide) in app.presentation.slides.iter().enumerate() {
        let is_active = i == app.current_slide;
        let num_style = Style::default().fg(FG_MUTED);

        // サムネイルアイコン（exec持ちは緑アイコン）
        let has_exec = slide.exec_count() > 0;
        let icon = if has_exec { "▶" } else {
            match slide.blocks.first().map(|b| &b.kind) {
                Some(BlockKind::Heading { level: 1 }) => "H1",
                Some(BlockKind::Heading { level: 2 }) => "H2",
                _ => " ≡",
            }
        };
        let icon_style = if has_exec {
            Style::default().fg(FG_EXEC)
        } else {
            Style::default().fg(FG_MUTED)
        };

        let title_style = if is_active {
            Style::default().fg(FG_ACTIVE_SLIDE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG_MUTED)
        };

        let bg = if is_active { BG_BLOCK_SEL } else { BG_SIDEBAR };

        let line = Line::from(vec![
            Span::styled(format!("{:>2} ", i + 1), num_style),
            Span::styled(format!("{:<3} ", icon), icon_style),
            Span::styled(
                truncate(&slide.title, 12),
                title_style,
            ),
        ]);

        items.push(
            ListItem::new(line)
                .style(Style::default().bg(bg))
        );
    }

    // 「+ スライド追加」エントリ
    items.push(ListItem::new(Line::from(vec![
        Span::styled(" +  ", Style::default().fg(FG_ACCENT)),
        Span::styled("new  ", Style::default().fg(FG_MUTED)),
        Span::styled("スライド追加", Style::default().fg(Color::Rgb(68, 68, 68))),
    ])).style(Style::default().bg(BG_SIDEBAR)));

    let mut state = ListState::default();
    state.select(Some(app.current_slide));

    f.render_stateful_widget(
        List::new(items).style(Style::default().bg(BG_SIDEBAR)),
        inner,
        &mut state,
    );
}

// ── キャンバス（スライド本体）────────────────────────────────────
fn draw_canvas(f: &mut Frame<'_>, app: &App, area: Rect) {
    // 背景
    f.render_widget(
        Block::default().style(Style::default().bg(BG_BASE)),
        area,
    );

    // スライドフレーム（余白付き）
    let margin_h = 3u16;
    let margin_v = 2u16;
    if area.width < margin_h * 2 + 4 || area.height < margin_v * 2 + 4 {
        return;
    }
    let slide_area = Rect {
        x: area.x + margin_h,
        y: area.y + margin_v,
        width: area.width.saturating_sub(margin_h * 2),
        height: area.height.saturating_sub(margin_v * 2),
    };

    // スライド背景+ボーダー
    let slide_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_DIM))
        .style(Style::default().bg(BG_SLIDE));
    f.render_widget(slide_block, slide_area);

    let inner = Rect {
        x: slide_area.x + 2,
        y: slide_area.y + 1,
        width: slide_area.width.saturating_sub(4),
        height: slide_area.height.saturating_sub(2),
    };

    draw_blocks(f, app, inner);
}

// ── ブロック描画 ─────────────────────────────────────────────────
fn draw_blocks(f: &mut Frame<'_>, app: &App, area: Rect) {
    let slide = app.current_slide();
    let mut y = area.y;

    for (i, block) in slide.blocks.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }
        let is_selected = app.selected_block == Some(i);
        let is_editing  = is_selected && app.mode == AppMode::EditingBlock;

        // ブロックの高さを計算
        let content = if is_editing { &app.edit_buffer } else { &block.content };
        let block_height = estimate_block_height(block, content, area.width);
        let block_height = block_height.min(area.y + area.height - y);

        let block_area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: block_height,
        };

        draw_single_block(f, app, block, i, is_selected, is_editing, content, block_area);
        y += block_height + 1; // ブロック間の隙間
    }
}

fn estimate_block_height(block: &crate::model::Block, content: &str, width: u16) -> u16 {
    match &block.kind {
        BlockKind::Heading { .. }    => 3,
        BlockKind::OutputPlaceholder => 3,
        BlockKind::Separator         => 1,
        _ => {
            let lines = content.lines().count().max(1);
            let wrapped = (content.len() as u16 / width.max(1)).max(0);
            (lines as u16 + wrapped + 2).min(8)
        }
    }
}

fn draw_single_block(
    f: &mut Frame<'_>,
    _app: &App,
    block: &crate::model::Block,
    _idx: usize,
    is_selected: bool,
    is_editing: bool,
    content: &str,
    area: Rect,
) {
    // ブロックの種別に応じた背景・ボーダー色
    let (bg, border_color) = match &block.kind {
        BlockKind::Exec { signature, .. } => {
            let is_signed = signature.is_some();
            if is_selected {
                (BG_EXEC_SEL, if is_signed { BORDER_EXEC_SEL } else { FG_WARN })
            } else {
                (BG_EXEC, BORDER_EXEC)
            }
        }
        BlockKind::OutputPlaceholder => (BG_SLIDE, Color::Rgb(42, 42, 42)),
        _ => {
            if is_selected {
                (BG_BLOCK_SEL, BORDER_SEL)
            } else {
                (BG_SLIDE, BORDER_DIM)
            }
        }
    };

    // ラベルテキスト（右上）
    let label = block.label();
    let label_style = match &block.kind {
        BlockKind::Exec { signature, .. } => {
            if signature.is_some() {
                Style::default().fg(FG_EXEC).bg(Color::Rgb(30, 50, 20))
            } else {
                Style::default().fg(FG_WARN).bg(Color::Rgb(50, 20, 20))
            }
        }
        _ => Style::default().fg(Color::Rgb(102, 102, 119)).bg(Color::Rgb(42, 42, 58)),
    };

    // ブロック外枠
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg));
    f.render_widget(outer, area);

    // ラベルを右上に描画
    if area.width > 10 {
        let label_len = label.chars().count() as u16 + 2;
        let label_x = area.x + area.width.saturating_sub(label_len + 1);
        let label_area = Rect { x: label_x, y: area.y, width: label_len, height: 1 };
        f.render_widget(
            Paragraph::new(format!(" {} ", label)).style(label_style),
            label_area,
        );
    }

    // 内側コンテンツ描画
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    match &block.kind {
        BlockKind::Heading { level } => {
            let size_mod = match level { 1 => Modifier::BOLD, _ => Modifier::empty() };
            let fg = match level { 1 => FG_PRIMARY, _ => FG_SECONDARY };
            let text = if is_editing {
                format!("{}▋", content)
            } else {
                content.to_string()
            };
            let widget = Paragraph::new(text)
                .style(Style::default().fg(fg).bg(bg).add_modifier(size_mod))
                .wrap(Wrap { trim: false });
            f.render_widget(widget, inner);
        }
        BlockKind::Exec { lang, .. } => {
            // langバッジ + コード
            let badge = Span::styled(
                format!(" {} ", lang),
                Style::default().fg(FG_EXEC).bg(Color::Rgb(30, 58, 20)),
            );
            let code = Span::styled(
                format!(" {}", content),
                Style::default().fg(FG_CODE).bg(bg),
            );
            let line = Line::from(vec![badge, code]);
            f.render_widget(Paragraph::new(vec![line]).style(Style::default().bg(bg)), inner);
        }
        BlockKind::OutputPlaceholder => {
            let placeholder = if content.is_empty() {
                "← 実行時にここに stdout が表示されます".to_string()
            } else {
                content.to_string()
            };
            let widget = Paragraph::new(placeholder)
                .style(Style::default().fg(Color::Rgb(68, 68, 68)).bg(bg)
                    .add_modifier(Modifier::ITALIC))
                .wrap(Wrap { trim: false });
            f.render_widget(widget, inner);
        }
        BlockKind::Separator => {
            let line = "─".repeat(inner.width as usize);
            f.render_widget(
                Paragraph::new(line).style(Style::default().fg(BORDER_DIM).bg(bg)),
                inner,
            );
        }
        _ => {
            // Text, Code など
            let text = if is_editing {
                format!("{}▋", content)
            } else {
                content.to_string()
            };
            let fg = match &block.kind {
                BlockKind::Code { .. } => FG_CODE,
                _ => FG_SECONDARY,
            };
            let widget = Paragraph::new(text)
                .style(Style::default().fg(fg).bg(bg))
                .wrap(Wrap { trim: false });
            f.render_widget(widget, inner);
        }
    }
}

// ── ツールバー ───────────────────────────────────────────────────
fn draw_toolbar(f: &mut Frame<'_>, _app: &App, area: Rect) {
    f.render_widget(
        Block::default().style(Style::default().bg(BG_TOOLBAR)),
        area,
    );

    let items: Vec<(&str, Color)> = vec![
        ("T 見出し", FG_ACCENT),
        ("¶ テキスト", FG_SECONDARY),
        ("⌥ コード", FG_SECONDARY),
        ("▶ exec", FG_EXEC),
        ("⊞ 画像", FG_SECONDARY),
        ("― 区切り", FG_SECONDARY),
        ("↑ 上へ", FG_SECONDARY),
        ("↓ 下へ", FG_SECONDARY),
        ("🔑 署名", FG_WARN),
        ("⚡ AI審査", FG_WARN),
        ("▷ プレゼン", FG_SECONDARY),
    ];

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw(" "));
    for (label, color) in &items {
        spans.push(Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(*color)
                .bg(Color::Rgb(30, 30, 30))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(BG_TOOLBAR)),
        area,
    );
}

// ── ステータスバー ───────────────────────────────────────────────
fn draw_statusbar(f: &mut Frame<'_>, app: &App, area: Rect) {
    f.render_widget(
        Block::default().style(Style::default().bg(BG_STATUSBAR)),
        area,
    );

    let mode_str = match app.mode {
        AppMode::Normal       => "編集モード",
        AppMode::EditingBlock => "テキスト編集",
        AppMode::BlockPicker  => "ブロック追加",
        AppMode::ExecConfirm  => "実行確認",
        AppMode::Present      => "プレゼン",
    };

    let exec_count   = app.total_exec_count();
    let signed_count = app.total_signed_count();
    let exec_str = if exec_count > 0 {
        format!("exec ×{}  署名済 ×{}", exec_count, signed_count)
    } else {
        String::new()
    };

    let slide_str = format!(
        "スライド {}/{}",
        app.current_slide + 1,
        app.slide_count()
    );

    let font_str = format!(
        "{}  {}px",
        app.presentation.font_name,
        app.presentation.font_size,
    );

    let help_str = "Ctrl+S 保存  h/l スライド  j/k ブロック  e 編集  n 追加  ? ヘルプ";

    let status_msg = app.status_message.as_deref().unwrap_or("");

    let spans = vec![
        Span::styled(format!(" {} ", mode_str), Style::default().fg(FG_ACCENT).bg(BG_STATUSBAR).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {} ", slide_str), Style::default().fg(FG_SECONDARY).bg(BG_STATUSBAR)),
        Span::styled(format!("  {} ", exec_str), Style::default().fg(FG_EXEC).bg(BG_STATUSBAR)),
        Span::styled(format!("  {} ", font_str), Style::default().fg(FG_SECONDARY).bg(BG_STATUSBAR)),
        Span::styled(format!("  {} ", if status_msg.is_empty() { help_str } else { status_msg }), Style::default().fg(FG_MUTED).bg(BG_STATUSBAR)),
    ];

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_STATUSBAR)),
        area,
    );
}

// ── ブロック追加パレット（オーバーレイ）────────────────────────
fn draw_block_picker(f: &mut Frame<'_>, _app: &App, area: Rect) {
    let w = 40u16.min(area.width);
    let h = 14u16.min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let popup_area = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" ブロックを追加 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(FG_ACCENT))
        .style(Style::default().bg(Color::Rgb(28, 28, 40)));
    f.render_widget(block, popup_area);

    let inner = Rect {
        x: popup_area.x + 2,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(4),
        height: popup_area.height.saturating_sub(2),
    };

    let items = vec![
        ("1", "H  ", "見出し (H1)", FG_PRIMARY),
        ("2", "¶  ", "テキスト", FG_SECONDARY),
        ("3", "{ }", "コードブロック", FG_CODE),
        ("4", "▶  ", "exec ブロック（実行可能）", FG_EXEC),
        ("5", "⊞  ", "出力プレースホルダ", FG_MUTED),
        ("6", "―  ", "区切り", FG_MUTED),
    ];

    let lines: Vec<Line> = items.iter().map(|(key, icon, label, color)| {
        Line::from(vec![
            Span::styled(format!(" {} ", key), Style::default().fg(FG_ACCENT)),
            Span::styled(format!("{}  ", icon), Style::default().fg(*color)),
            Span::styled(label.to_string(), Style::default().fg(FG_SECONDARY)),
        ])
    }).collect();

    let hint = Line::from(vec![
        Span::styled("  Esc ", Style::default().fg(FG_MUTED)),
        Span::styled("キャンセル", Style::default().fg(Color::Rgb(68, 68, 68))),
    ]);

    let mut all_lines = lines;
    all_lines.push(Line::from(""));
    all_lines.push(hint);

    f.render_widget(
        Paragraph::new(Text::from(all_lines))
            .style(Style::default().bg(Color::Rgb(28, 28, 40))),
        inner,
    );
}

// ── ユーティリティ ───────────────────────────────────────────────
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let mut result: String = chars[..max_chars - 1].iter().collect();
        result.push('…');
        result
    }
}

// ── プレゼンモード（全画面）────────────────────────────────────
pub fn draw_present(f: &mut Frame<'_>, app: &App) {
    let area = f.size();

    f.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(18, 18, 18))),
        area,
    );

    // スライドコンテンツを中央に大きく表示
    let slide = app.current_slide();
    let mut lines: Vec<Line> = Vec::new();

    for block in &slide.blocks {
        match &block.kind {
            BlockKind::Heading { level } => {
                let prefix = match level { 1 => "", 2 => "  ", _ => "    " };
                let style = match level {
                    1 => Style::default().fg(Color::Rgb(240,240,240)).add_modifier(Modifier::BOLD),
                    _ => Style::default().fg(Color::Rgb(180,180,180)).add_modifier(Modifier::BOLD),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{}{}", prefix, block.content), style)
                ]));
                lines.push(Line::from(""));
            }
            BlockKind::Text => {
                for l in block.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}", l), Style::default().fg(Color::Rgb(180,180,180)))
                    ]));
                }
                lines.push(Line::from(""));
            }
            BlockKind::Code { lang } | BlockKind::Exec { lang, .. } => {
                let is_exec = block.is_exec();
                let header_color = if is_exec { FG_EXEC } else { FG_CODE };
                let tag = if is_exec { "▶ exec" } else { lang.as_str() };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", tag), Style::default().fg(header_color))
                ]));
                for l in block.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled(format!("    {}", l), Style::default().fg(FG_CODE))
                    ]));
                }
                lines.push(Line::from(""));
            }
            BlockKind::OutputPlaceholder => {
                if !block.content.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  [出力]", Style::default().fg(FG_MUTED))
                    ]));
                    for l in block.content.lines().take(10) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {}", l), Style::default().fg(Color::Rgb(140,200,140)))
                        ]));
                    }
                    lines.push(Line::from(""));
                }
            }
            BlockKind::Separator => {
                lines.push(Line::from(vec![
                    Span::styled("  ────────────────────────", Style::default().fg(BORDER_DIM))
                ]));
                lines.push(Line::from(""));
            }
        }
    }

    // コンテンツ領域（上下にマージン）
    let content_area = Rect {
        x: area.x + area.width / 6,
        y: area.y + area.height / 5,
        width: area.width * 2 / 3,
        height: area.height * 3 / 5,
    };

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(Color::Rgb(18,18,18))),
        content_area,
    );

    // スライド番号（右下）
    let page_str = format!(" {}/{} ", app.current_slide + 1, app.slide_count());
    let page_area = Rect {
        x: area.x + area.width - page_str.len() as u16 - 2,
        y: area.y + area.height - 1,
        width: page_str.len() as u16 + 2,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(page_str).style(Style::default().fg(FG_MUTED).bg(Color::Rgb(18,18,18))),
        page_area,
    );

    // 操作ヒント（左下）
    let hint_area = Rect { x: area.x + 1, y: area.y + area.height - 1, width: 40, height: 1 };
    f.render_widget(
        Paragraph::new(" h/l: スライド移動  Esc: 終了")
            .style(Style::default().fg(Color::Rgb(50,50,50)).bg(Color::Rgb(18,18,18))),
        hint_area,
    );
}

// ── exec実行確認ダイアログ ───────────────────────────────────
pub fn draw_exec_confirm(f: &mut Frame<'_>, app: &App) {
    // 背景は通常のエディタUIを描画してからオーバーレイ
    draw(f, app);

    let area = f.size();
    let w = 60u16.min(area.width - 4);
    let h = 12u16;
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);

    let outer = Block::default()
        .title(" ⚡ exec 実行確認 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(FG_EXEC))
        .style(Style::default().bg(Color::Rgb(18, 28, 18)));
    f.render_widget(outer, popup);

    let inner = Rect {
        x: popup.x + 2,
        y: popup.y + 1,
        width: popup.width.saturating_sub(4),
        height: popup.height.saturating_sub(2),
    };

    if let Some(block) = app.selected_block_ref() {
        let cmd_line = Line::from(vec![
            Span::styled("  $ ", Style::default().fg(FG_EXEC)),
            Span::styled(block.content.as_str(), Style::default().fg(FG_CODE)),
        ]);
        let signed_line = if block.is_signed() {
            Line::from(vec![
                Span::styled("  ✓ 署名済み", Style::default().fg(FG_EXEC)),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ✗ 未署名", Style::default().fg(FG_WARN)),
            ])
        };
        let confirm_line = Line::from(vec![
            Span::styled("  [y] 実行  ", Style::default().fg(FG_PRIMARY)),
            Span::styled("[n / Esc] キャンセル", Style::default().fg(FG_MUTED)),
        ]);

        let text = Text::from(vec![
            Line::from(""),
            cmd_line,
            Line::from(""),
            signed_line,
            Line::from(""),
            Line::from(""),
            confirm_line,
        ]);

        f.render_widget(
            Paragraph::new(text).style(Style::default().bg(Color::Rgb(18, 28, 18))),
            inner,
        );
    }
}
