use crate::app::{App, AppMode};
use crate::model::BlockKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub enum Action {
    None,
    Quit,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    match app.mode {
        AppMode::Normal => handle_normal(app, key),
        AppMode::EditingBlock => handle_editing(app, key),
        AppMode::BlockPicker => handle_picker(app, key),
        AppMode::ExecConfirm => handle_exec_confirm(app, key),
        AppMode::Settings => handle_settings(app, key),
        AppMode::CommandInput => handle_command_input(app, key),
        AppMode::PresentExecConfirm => handle_present_exec_confirm(app, key),
        AppMode::Present => handle_present(app, key),
    }
}

// ── ノーマルモード（ナビゲーション） ─────────────────────────
fn handle_normal(app: &mut App, key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('s') => {
                app.set_status("保存しました（未実装）");
                return Action::None;
            }
            KeyCode::Char('q') => {
                app.enter_settings();
                return Action::None;
            }
            KeyCode::Char('c') => return Action::Quit,
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => return Action::Quit,

        // スライドナビ
        KeyCode::Char('h') | KeyCode::Left => app.prev_slide(),
        KeyCode::Char('l') | KeyCode::Right => app.next_slide(),

        // ブロックナビ
        KeyCode::Char('k') | KeyCode::Up => app.prev_block(),
        KeyCode::Char('j') | KeyCode::Down => app.next_block(),

        // 編集（テキスト・コード・execのコマンド編集）
        KeyCode::Char('e') | KeyCode::Enter => app.start_edit(),

        // ブロック追加パレット
        KeyCode::Char('n') => app.mode = AppMode::BlockPicker,

        // ブロック移動
        KeyCode::Char('K') => app.move_block_up(),
        KeyCode::Char('J') => app.move_block_down(),

        // スライド追加
        KeyCode::Char('N') => app.add_slide(),

        // ── exec専用操作 ─────────────────────────────
        // Space: execブロック実行（署名済みなら確認ダイアログ、未署名なら警告）
        KeyCode::Char(' ') => app.try_exec_selected(),

        // s: 選択中のexecブロックに署名
        KeyCode::Char('s') => app.sign_selected(),

        // c: 実行中のexecをキャンセル
        KeyCode::Char('c') => app.cancel_running_exec(),

        // [ / PgUp: 出力スクロール上
        KeyCode::Char('[') | KeyCode::PageUp => app.scroll_output_up(),
        // ] / PgDn: 出力スクロール下
        KeyCode::Char(']') | KeyCode::PageDown => app.scroll_output_down(),

        // a: AI審査（未実装・スタブ）
        KeyCode::Char('a') => {
            if app.is_exec_selected() {
                app.set_status("AI審査: 未実装（ANTHROPIC_API_KEY があれば呼び出します）");
            } else {
                app.set_status("execブロックを選択してください");
            }
        }

        // ── プレゼンモード ────────────────────────────
        KeyCode::Char('p') => app.enter_present(),

        // : コマンド
        KeyCode::Char(':') => app.start_command(),

        // 数字キーでスライドジャンプ
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let n = c as usize - '0' as usize;
            app.go_to_slide(n - 1);
        }

        KeyCode::Esc => {
            app.status_message = None;
        }

        _ => {}
    }
    Action::None
}

// ── テキスト編集モード ────────────────────────────────────────
fn handle_editing(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => app.cancel_edit(),
        KeyCode::Backspace => app.delete_char_before(),
        KeyCode::Left => app.cursor_left(),
        KeyCode::Right => app.cursor_right(),
        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.insert_char('\n');
            } else {
                app.commit_edit();
            }
        }
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if c == 'c' {
                app.cancel_edit();
            }
        }
        KeyCode::Char(c) => app.insert_char(c),
        _ => {}
    }
    Action::None
}

// ── ブロック追加パレット ─────────────────────────────────────
fn handle_picker(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => app.mode = AppMode::Normal,
        KeyCode::Char('1') => {
            app.add_block(BlockKind::Heading { level: 1 });
            app.start_edit();
        }
        KeyCode::Char('2') => {
            app.add_block(BlockKind::Text);
            app.start_edit();
        }
        KeyCode::Char('3') => {
            app.add_block(BlockKind::Code {
                lang: "rust".into(),
            });
            app.start_edit();
        }
        KeyCode::Char('4') => {
            app.add_block(BlockKind::Exec {
                lang: "bash".into(),
                signature: None,
            });
            // execブロック追加直後は署名が必要と案内
            app.set_status(
                "execブロックを追加しました。'e'でコマンド編集、's'で署名してから Space で実行",
            );
            app.start_edit();
        }
        KeyCode::Char('5') => {
            app.add_block(BlockKind::OutputPlaceholder);
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('6') => {
            app.add_block(BlockKind::Separator);
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Action::None
}

// ── exec実行確認ダイアログ ───────────────────────────────────
fn handle_exec_confirm(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        // y / Enter → 実行
        KeyCode::Char('y') | KeyCode::Enter => app.run_exec_selected(),
        // n / Esc → キャンセル
        KeyCode::Char('n') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.set_status("実行をキャンセルしました");
        }
        _ => {}
    }
    Action::None
}

// ── プレゼン中exec実行確認ダイアログ ─────────────────────────────
fn handle_present_exec_confirm(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        // y / Enter → 実行してプレゼンへ戻る（popup 閉鎖後の背景欠けを防ぐため needs_redraw も立てる）
        KeyCode::Char('y') | KeyCode::Enter => app.confirm_present_exec_run(),
        // n / Esc → キャンセルしてプレゼンへ戻る
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_present_exec_dialog(),
        _ => {}
    }
    Action::None
}

// ── 設定画面 ────────────────────────────────────────────────
fn handle_settings(app: &mut App, key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q') => app.exit_settings(),
            KeyCode::Char('c') => return Action::Quit,
            _ => {}
        }
        return Action::None;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.exit_settings(),
        KeyCode::Char(':') => app.start_command(),
        KeyCode::Char('+') | KeyCode::Char('=') => app.increase_font_size(),
        KeyCode::Char('-') => app.decrease_font_size(),
        KeyCode::Char('f') => app.cycle_font_name(),
        KeyCode::Char('t') => app.cycle_theme(),
        KeyCode::Char('a') => app.cycle_accent(),
        _ => {}
    }
    Action::None
}

// ── : コマンド入力 ───────────────────────────────────────────
fn handle_command_input(app: &mut App, key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => app.cancel_command(),
            _ => {}
        }
        return Action::None;
    }

    match key.code {
        KeyCode::Esc => app.cancel_command(),
        KeyCode::Enter => app.commit_command(),
        KeyCode::Backspace => app.delete_command_char_before(),
        KeyCode::Left => app.command_cursor_left(),
        KeyCode::Right => app.command_cursor_right(),
        KeyCode::Char(c) => app.insert_command_char(c),
        _ => {}
    }
    Action::None
}

// ── プレゼンモード ────────────────────────────────────────────
fn handle_present(app: &mut App, key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q') => {
                app.enter_settings();
                return Action::None;
            }
            KeyCode::Char('c') => return Action::Quit,
            _ => {}
        }
    }

    match key.code {
        // 次スライド
        KeyCode::Char('l') | KeyCode::Right => app.next_slide(),

        // exec選択中は実行確認、それ以外は次スライド
        KeyCode::Char(' ') | KeyCode::Enter => {
            if app.is_exec_selected() {
                app.try_present_exec_selected();
            } else {
                app.next_slide();
            }
        }

        // 前スライド
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => app.prev_slide(),

        // ブロック選択
        KeyCode::Char('k') | KeyCode::Up => app.prev_block(),
        KeyCode::Char('j') | KeyCode::Down => app.next_block(),

        // exec補助操作
        KeyCode::Char('s') => app.sign_selected(),
        KeyCode::Char('c') => app.cancel_running_exec(),
        KeyCode::Char('[') | KeyCode::PageUp => app.scroll_output_up(),
        KeyCode::Char(']') | KeyCode::PageDown => app.scroll_output_down(),
        KeyCode::Char('+') | KeyCode::Char('=') => app.increase_font_size(),
        KeyCode::Char('-') => app.decrease_font_size(),
        KeyCode::Char(':') => app.start_command(),

        // プレゼン終了
        KeyCode::Esc | KeyCode::Char('q') => app.exit_present(),

        _ => {}
    }
    Action::None
}
