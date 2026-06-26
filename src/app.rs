use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

use crate::markdown;
use crate::model::{Block, BlockKind, Presentation};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,             // ナビゲーション（ブロック選択・スライド移動）
    EditingBlock,       // テキスト編集中
    BlockPicker,        // ブロック追加パレット
    ExecConfirm,        // exec実行確認オーバーレイ
    Settings,           // 設定画面
    CommandInput,       // : コマンド入力
    PresentExecConfirm, // プレゼン中exec実行確認オーバーレイ
    Present,            // プレゼンモード（全画面）
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiTheme {
    Dark,
    Light,
    HighContrast,
}

impl UiTheme {
    pub fn label(self) -> &'static str {
        match self {
            UiTheme::Dark => "dark",
            UiTheme::Light => "light",
            UiTheme::HighContrast => "high-contrast",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dark" => Some(UiTheme::Dark),
            "light" => Some(UiTheme::Light),
            "high-contrast" | "highcontrast" | "contrast" => Some(UiTheme::HighContrast),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            UiTheme::Dark => UiTheme::Light,
            UiTheme::Light => UiTheme::HighContrast,
            UiTheme::HighContrast => UiTheme::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccentColor {
    Blue,
    Green,
    Pink,
    Yellow,
    Red,
}

impl AccentColor {
    pub fn label(self) -> &'static str {
        match self {
            AccentColor::Blue => "blue",
            AccentColor::Green => "green",
            AccentColor::Pink => "pink",
            AccentColor::Yellow => "yellow",
            AccentColor::Red => "red",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "blue" => Some(AccentColor::Blue),
            "green" => Some(AccentColor::Green),
            "pink" | "magenta" => Some(AccentColor::Pink),
            "yellow" => Some(AccentColor::Yellow),
            "red" => Some(AccentColor::Red),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            AccentColor::Blue => AccentColor::Green,
            AccentColor::Green => AccentColor::Pink,
            AccentColor::Pink => AccentColor::Yellow,
            AccentColor::Yellow => AccentColor::Red,
            AccentColor::Red => AccentColor::Blue,
        }
    }
}

pub enum ExecEvent {
    Stdout(String),
    Stderr(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecStatus {
    Running,
    Completed(i32),
    Failed(i32),
    Cancelled,
    SpawnError,
}

pub struct RunningExec {
    pub block_id: u64,
    pub slide_idx: usize,
    pub placeholder_idx: Option<usize>,
    pub child: Option<Child>,
    pub rx: Receiver<ExecEvent>,
    pub buffer: String,
    pub scroll: u16,
    pub status: ExecStatus,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub notified: bool,
}

pub struct App {
    pub presentation: Presentation,
    pub current_file: Option<PathBuf>,
    pub present_reveal_step: usize,
    pub current_slide: usize,
    pub selected_block: Option<usize>,
    pub mode: AppMode,
    pub edit_buffer: String,
    pub edit_cursor: usize,
    pub command_buffer: String,
    pub command_cursor: usize,
    pub command_return_mode: AppMode,
    pub settings_return_mode: AppMode,
    pub status_message: Option<String>,
    pub next_block_id: u64,
    pub running_exec: Option<RunningExec>,
    pub ui_theme: UiTheme,
    pub accent_color: AccentColor,
}

impl App {
    pub fn new(presentation: Presentation) -> Self {
        Self::with_file(presentation, None)
    }

    pub fn with_file(presentation: Presentation, current_file: Option<PathBuf>) -> Self {
        let next_id = Self::next_block_id_for(&presentation);
        let selected_block = presentation
            .slides
            .first()
            .and_then(|slide| if slide.blocks.is_empty() { None } else { Some(0) });
        Self {
            presentation,
            current_file,
            present_reveal_step: 0,
            current_slide: 0,
            selected_block,
            mode: AppMode::Normal,
            edit_buffer: String::new(),
            edit_cursor: 0,
            command_buffer: String::new(),
            command_cursor: 0,
            command_return_mode: AppMode::Normal,
            settings_return_mode: AppMode::Normal,
            status_message: None,
            next_block_id: next_id,
            running_exec: None,
            ui_theme: UiTheme::Dark,
            accent_color: AccentColor::Blue,
        }
    }

    fn next_block_id_for(presentation: &Presentation) -> u64 {
        presentation
            .slides
            .iter()
            .flat_map(|s| s.blocks.iter())
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn current_slide(&self) -> &crate::model::Slide {
        &self.presentation.slides[self.current_slide]
    }

    pub fn current_slide_mut(&mut self) -> &mut crate::model::Slide {
        &mut self.presentation.slides[self.current_slide]
    }

    pub fn slide_count(&self) -> usize {
        self.presentation.slides.len()
    }

    // 現在選択中のブロック
    pub fn selected_block_ref(&self) -> Option<&crate::model::Block> {
        self.selected_block
            .and_then(|i| self.current_slide().blocks.get(i))
    }

    pub fn is_exec_selected(&self) -> bool {
        self.selected_block_ref()
            .map(|b| b.is_exec())
            .unwrap_or(false)
    }

    pub fn is_signed_selected(&self) -> bool {
        self.selected_block_ref()
            .map(|b| b.is_signed())
            .unwrap_or(false)
    }

    // ── ナビゲーション ──────────────────────────────

    pub fn prev_slide(&mut self) {
        if self.current_slide > 0 {
            self.current_slide -= 1;
            self.present_reveal_step = 0;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn next_slide(&mut self) {
        if self.current_slide + 1 < self.presentation.slides.len() {
            self.current_slide += 1;
            self.present_reveal_step = 0;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn go_to_slide(&mut self, idx: usize) {
        if idx < self.presentation.slides.len() {
            self.current_slide = idx;
            self.present_reveal_step = 0;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn prev_block(&mut self) {
        if let Some(sel) = self.selected_block {
            if sel > 0 {
                self.selected_block = Some(sel - 1);
            }
        }
    }

    pub fn next_block(&mut self) {
        let len = self.current_slide().blocks.len();
        match self.selected_block {
            Some(sel) if sel + 1 < len => self.selected_block = Some(sel + 1),
            None if len > 0 => self.selected_block = Some(0),
            _ => {}
        }
    }

    // ── 編集 ────────────────────────────────────────

    pub fn start_edit(&mut self) {
        if let Some(sel) = self.selected_block {
            // execブロックはテキスト編集可（コマンドを変える）
            let content = self.current_slide().blocks[sel].content.clone();
            self.edit_buffer = content.clone();
            self.edit_cursor = content.len();
            self.mode = AppMode::EditingBlock;
        }
    }

    pub fn commit_edit(&mut self) {
        if let Some(sel) = self.selected_block {
            // 実行中ブロックの編集はブロック
            let block_id = self.current_slide().blocks[sel].id;
            if let Some(running) = &self.running_exec {
                if running.block_id == block_id && matches!(running.status, ExecStatus::Running) {
                    self.set_status(
                        "実行中のブロックは編集できません ('c' でキャンセル)".to_string(),
                    );
                    self.mode = AppMode::Normal;
                    return;
                }
            }
            let buf = self.edit_buffer.clone();
            let slide = self.current_slide_mut();
            let content_changed = slide.blocks[sel].content != buf;
            // コマンドを変えたらexecブロックの署名をリセット
            if content_changed {
                if let BlockKind::Exec {
                    ref mut signature, ..
                } = slide.blocks[sel].kind
                {
                    *signature = None;
                }
            }
            slide.blocks[sel].content = buf;
        }
        self.mode = AppMode::Normal;
    }

    pub fn cancel_edit(&mut self) {
        self.mode = AppMode::Normal;
    }

    pub fn insert_char(&mut self, ch: char) {
        let cursor = self.edit_cursor;
        self.edit_buffer.insert(cursor, ch);
        self.edit_cursor += ch.len_utf8();
    }

    pub fn delete_char_before(&mut self) {
        if self.edit_cursor > 0 {
            let cursor = self.edit_cursor;
            let mut new_cursor = cursor - 1;
            while !self.edit_buffer.is_char_boundary(new_cursor) {
                new_cursor -= 1;
            }
            self.edit_buffer.drain(new_cursor..cursor);
            self.edit_cursor = new_cursor;
        }
    }

    pub fn cursor_left(&mut self) {
        if self.edit_cursor > 0 {
            self.edit_cursor -= 1;
            while !self.edit_buffer.is_char_boundary(self.edit_cursor) {
                self.edit_cursor -= 1;
            }
        }
    }

    pub fn cursor_right(&mut self) {
        if self.edit_cursor < self.edit_buffer.len() {
            self.edit_cursor += 1;
            while !self.edit_buffer.is_char_boundary(self.edit_cursor) {
                self.edit_cursor += 1;
            }
        }
    }

    // ── exec 操作 ────────────────────────────────────

    /// execブロックを選択中のとき Space → 実行確認ダイアログへ
    pub fn try_exec_selected(&mut self) {
        if self.is_exec_selected() {
            if self.is_signed_selected() {
                self.mode = AppMode::ExecConfirm;
            } else {
                // 未署名 → 署名フローを促す
                self.set_status(
                    "未署名のexecブロックです。's'で署名、'a'でAI審査してください".to_string(),
                );
            }
        }
    }

    /// プレゼン中にexecブロックを選択中のとき Space / Enter → 実行確認へ
    pub fn try_present_exec_selected(&mut self) {
        if self.is_exec_selected() {
            if self.is_signed_selected() {
                self.mode = AppMode::PresentExecConfirm;
            } else {
                self.set_status(
                    "未署名のexecブロックです。's'で署名してから実行してください".to_string(),
                );
            }
        }
    }

    /// 選択中のexecブロックに署名（簡易実装: ダミー署名）
    pub fn sign_selected(&mut self) {
        if let Some(sel) = self.selected_block {
            let slide = self.current_slide_mut();
            let is_exec = matches!(slide.blocks[sel].kind, BlockKind::Exec { .. });
            if is_exec {
                let content_len = slide.blocks[sel].content.len();
                let hash = format!("sig:ed25519:mock:{:x}", content_len * 31 + 0xdeadbeef);
                if let BlockKind::Exec {
                    ref mut signature, ..
                } = slide.blocks[sel].kind
                {
                    *signature = Some(hash);
                }
                self.set_status("署名しました（開発用モック署名）".to_string());
            } else {
                self.set_status("execブロックを選択してください".to_string());
            }
        }
    }

    /// exec実行（署名済みのみ）。サブプロセスをspawnしてワーカースレッドで出力を吸い上げる。
    pub fn run_exec_selected(&mut self) {
        let sel = match self.selected_block {
            Some(s) => s,
            None => {
                self.mode = AppMode::Normal;
                return;
            }
        };
        if !self.is_signed_selected() {
            self.set_status("署名されていません。's'で署名してください".to_string());
            self.mode = AppMode::Normal;
            return;
        }
        // 走行中ならブロック。終了済みなら破棄して新規実行可能に
        if let Some(running) = &self.running_exec {
            if matches!(running.status, ExecStatus::Running) {
                self.set_status("別のexecが実行中です。'c'でキャンセルしてください".to_string());
                self.mode = AppMode::Normal;
                return;
            }
        }
        self.running_exec = None;

        let slide_idx = self.current_slide;
        let block = &self.current_slide().blocks[sel];
        let block_id = block.id;
        let cmd = block.content.clone();

        // 直後のOutputPlaceholderを探す（無くてもRunningExec.bufferには蓄積する）
        let placeholder_idx = self.current_slide().blocks.get(sel + 1).and_then(|b| {
            if matches!(b.kind, BlockKind::OutputPlaceholder) {
                Some(sel + 1)
            } else {
                None
            }
        });

        // 既存の出力をクリア
        if let Some(idx) = placeholder_idx {
            self.current_slide_mut().blocks[idx].content.clear();
        }

        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                self.set_status(format!("[実行エラー] {}", e));
                self.mode = AppMode::Normal;
                return;
            }
        };

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let (tx, rx) = mpsc::channel::<ExecEvent>();

        // stdout reader thread
        let tx_out = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if tx_out.send(ExecEvent::Stdout(line)).is_err() {
                    break;
                }
            }
        });
        // stderr reader thread
        let tx_err = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                if tx_err.send(ExecEvent::Stderr(line)).is_err() {
                    break;
                }
            }
        });

        self.running_exec = Some(RunningExec {
            block_id,
            slide_idx,
            placeholder_idx,
            child: Some(child),
            rx,
            buffer: String::new(),
            scroll: 0,
            status: ExecStatus::Running,
            started_at: Instant::now(),
            finished_at: None,
            notified: false,
        });

        // wait用スレッドを別途立てない: poll_exec_events内で try_wait() でチェック
        // （tx は readersにcloneされて残り、両reader終了でdropされる。Finishedはmainで生成）
        drop(tx);

        self.set_status(format!("実行開始: {}", cmd));
        self.mode = AppMode::Normal;
    }

    /// 毎フレーム呼ぶ。サブプロセスからの出力イベントをdrainし、状態を更新。
    pub fn poll_exec_events(&mut self) {
        let Some(running) = self.running_exec.as_mut() else {
            return;
        };

        // チャネルからイベントをdrain
        let mut new_lines: Vec<String> = Vec::new();
        loop {
            match running.rx.try_recv() {
                Ok(ExecEvent::Stdout(line)) | Ok(ExecEvent::Stderr(line)) => {
                    new_lines.push(line);
                }
                Err(_) => break,
            }
        }

        for line in &new_lines {
            if !running.buffer.is_empty() {
                running.buffer.push('\n');
            }
            running.buffer.push_str(line);
        }

        // プロセス終了判定
        if matches!(running.status, ExecStatus::Running) {
            if let Some(child) = running.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code().unwrap_or(-1);
                        running.status = if status.success() {
                            ExecStatus::Completed(code)
                        } else {
                            ExecStatus::Failed(code)
                        };
                        running.finished_at = Some(Instant::now());
                        running.child = None;
                    }
                    Ok(None) => {} // まだ走ってる
                    Err(_) => {
                        running.status = ExecStatus::SpawnError;
                        running.finished_at = Some(Instant::now());
                        running.child = None;
                    }
                }
            }
        }

        // 終了直後にチャネルに残った行も拾うため、Finished後にもう一度drain
        if !matches!(running.status, ExecStatus::Running) && running.child.is_none() {
            // 残り行を吸い上げ
            loop {
                match running.rx.try_recv() {
                    Ok(ExecEvent::Stdout(line)) | Ok(ExecEvent::Stderr(line)) => {
                        if !running.buffer.is_empty() {
                            running.buffer.push('\n');
                        }
                        running.buffer.push_str(&line);
                    }
                    _ => break,
                }
            }
        }

        // OutputPlaceholderに反映
        let slide_idx = running.slide_idx;
        let placeholder_idx = running.placeholder_idx;
        let buffer_snapshot = running.buffer.clone();
        let just_finished = !matches!(running.status, ExecStatus::Running)
            && running.finished_at.is_some()
            && new_lines.is_empty()
            && running.child.is_none();

        if let Some(idx) = placeholder_idx {
            if let Some(slide) = self.presentation.slides.get_mut(slide_idx) {
                if let Some(block) = slide.blocks.get_mut(idx) {
                    block.content = buffer_snapshot;
                }
            }
        }

        // 終了したらステータスメッセージを更新（一度だけ）
        let running = self.running_exec.as_mut().unwrap();
        if just_finished && !running.notified {
            running.notified = true;
            let msg = match running.status {
                ExecStatus::Completed(c) => format!("実行完了 (exit {})", c),
                ExecStatus::Failed(c) => format!("実行失敗 (exit {})", c),
                ExecStatus::Cancelled => "キャンセルしました".to_string(),
                ExecStatus::SpawnError => "実行エラー".to_string(),
                ExecStatus::Running => return,
            };
            self.set_status(msg);
        }
    }

    /// 実行中のexecをキャンセル（child.kill）
    pub fn cancel_running_exec(&mut self) {
        let Some(running) = self.running_exec.as_mut() else {
            self.set_status("実行中のexecはありません".to_string());
            return;
        };
        if !matches!(running.status, ExecStatus::Running) {
            return;
        }

        if let Some(child) = running.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        running.child = None;
        running.status = ExecStatus::Cancelled;
        running.finished_at = Some(Instant::now());
        running.notified = true;
        if !running.buffer.is_empty() {
            running.buffer.push('\n');
        }
        running.buffer.push_str("[キャンセルされました]");

        let slide_idx = running.slide_idx;
        let placeholder_idx = running.placeholder_idx;
        let snapshot = running.buffer.clone();
        if let Some(idx) = placeholder_idx {
            if let Some(slide) = self.presentation.slides.get_mut(slide_idx) {
                if let Some(block) = slide.blocks.get_mut(idx) {
                    block.content = snapshot;
                }
            }
        }
        self.set_status("キャンセルしました".to_string());
    }

    /// 出力をスクロール
    pub fn scroll_output_up(&mut self) {
        if let Some(running) = self.running_exec.as_mut() {
            running.scroll = running.scroll.saturating_sub(1);
        }
    }

    pub fn scroll_output_down(&mut self) {
        if let Some(running) = self.running_exec.as_mut() {
            let total_lines = running.buffer.lines().count() as u16;
            // 上限は緩く: 最終行から1行手前まで（描画側でクランプされる前提）
            if running.scroll + 1 < total_lines {
                running.scroll += 1;
            }
        }
    }

    // ── ブロック追加・削除・移動 ─────────────────────

    pub fn add_block(&mut self, kind: BlockKind) {
        let id = self.next_block_id;
        self.next_block_id += 1;
        let block = Block::new(id, kind, "");
        let insert_pos = self
            .selected_block
            .map(|s| s + 1)
            .unwrap_or(self.current_slide().blocks.len());
        self.current_slide_mut().blocks.insert(insert_pos, block);
        self.selected_block = Some(insert_pos);
        self.mode = AppMode::Normal;
    }

    pub fn add_slide(&mut self) {
        let new_slide = crate::model::Slide::new("新しいスライド");
        let pos = self.current_slide + 1;
        self.presentation.slides.insert(pos, new_slide);
        self.current_slide = pos;
        self.selected_block = None;
    }

    pub fn move_block_up(&mut self) {
        if let Some(sel) = self.selected_block {
            if sel > 0 {
                self.current_slide_mut().blocks.swap(sel, sel - 1);
                self.selected_block = Some(sel - 1);
            }
        }
    }

    pub fn move_block_down(&mut self) {
        if let Some(sel) = self.selected_block {
            let len = self.current_slide().blocks.len();
            if sel + 1 < len {
                self.current_slide_mut().blocks.swap(sel, sel + 1);
                self.selected_block = Some(sel + 1);
            }
        }
    }

    // ── プレゼンモード ───────────────────────────────

    pub fn enter_present(&mut self) {
        self.go_to_slide(0);
        self.present_reveal_step = 0;
        self.mode = AppMode::Present;
    }

    pub fn exit_present(&mut self) {
        self.present_reveal_step = 0;
        self.mode = AppMode::Normal;
    }

    pub fn current_slide_reveal_count(&self) -> usize {
        self.current_slide()
            .blocks
            .iter()
            .filter(|block| matches!(block.kind, BlockKind::Text))
            .map(|block| markdown::reveal_marker_count(&block.content))
            .sum()
    }

    pub fn present_forward(&mut self) {
        if self.present_reveal_step < self.current_slide_reveal_count() {
            self.present_reveal_step += 1;
        } else {
            self.next_slide();
        }
    }

    pub fn present_backward(&mut self) {
        if self.present_reveal_step > 0 {
            self.present_reveal_step -= 1;
        } else if self.current_slide > 0 {
            self.current_slide -= 1;
            self.present_reveal_step = self.current_slide_reveal_count();
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    // ── 設定・コマンド ───────────────────────────────

    pub fn enter_settings(&mut self) {
        if !matches!(self.mode, AppMode::Settings | AppMode::CommandInput) {
            self.settings_return_mode = self.mode.clone();
        }
        self.mode = AppMode::Settings;
    }

    pub fn exit_settings(&mut self) {
        self.mode = self.settings_return_mode.clone();
    }

    pub fn start_command(&mut self) {
        if !matches!(self.mode, AppMode::CommandInput) {
            self.command_return_mode = self.mode.clone();
        }
        self.command_buffer.clear();
        self.command_cursor = 0;
        self.mode = AppMode::CommandInput;
    }

    pub fn cancel_command(&mut self) {
        self.mode = self.command_return_mode.clone();
    }

    pub fn insert_command_char(&mut self, ch: char) {
        let cursor = self.command_cursor;
        self.command_buffer.insert(cursor, ch);
        self.command_cursor += ch.len_utf8();
    }

    pub fn delete_command_char_before(&mut self) {
        if self.command_cursor > 0 {
            let cursor = self.command_cursor;
            let mut new_cursor = cursor - 1;
            while !self.command_buffer.is_char_boundary(new_cursor) {
                new_cursor -= 1;
            }
            self.command_buffer.drain(new_cursor..cursor);
            self.command_cursor = new_cursor;
        }
    }

    pub fn command_cursor_left(&mut self) {
        if self.command_cursor > 0 {
            self.command_cursor -= 1;
            while !self.command_buffer.is_char_boundary(self.command_cursor) {
                self.command_cursor -= 1;
            }
        }
    }

    pub fn command_cursor_right(&mut self) {
        if self.command_cursor < self.command_buffer.len() {
            self.command_cursor += 1;
            while !self.command_buffer.is_char_boundary(self.command_cursor) {
                self.command_cursor += 1;
            }
        }
    }

    pub fn commit_command(&mut self) {
        let raw = self.command_buffer.trim().to_string();
        let return_mode = self.command_return_mode.clone();

        if raw.is_empty() {
            self.mode = return_mode;
            return;
        }

        if matches!(raw.as_str(), "settings" | "config") {
            self.mode = return_mode;
            self.enter_settings();
            self.set_status("設定画面を開きました".to_string());
            return;
        }

        let result = self.apply_command(&raw);
        self.mode = return_mode;
        match result {
            Ok(msg) => self.set_status(msg),
            Err(msg) => self.set_status(msg),
        }
    }

    fn apply_command(&mut self, raw: &str) -> Result<String, String> {
        let mut parts = raw.split_whitespace();
        let Some(command) = parts.next() else {
            return Ok(String::new());
        };
        let value = parts.collect::<Vec<_>>().join(" ");

        match command {
            "w" | "write" | "save" => {
                if value.is_empty() {
                    self.save_current_file()
                } else {
                    self.save_to_path(PathBuf::from(value))
                }
            }
            "o" | "open" | "edit" => {
                if value.is_empty() {
                    return Err("open する Markdown ファイルを指定してください".to_string());
                }
                self.open_path(PathBuf::from(value))
            }
            "font-size" | "fontsize" => {
                let size = value
                    .parse::<u8>()
                    .map_err(|_| "font-size は数値で指定してください: :font-size 20".to_string())?;
                self.set_font_size(size);
                Ok(format!(
                    "font-size を {} にしました",
                    self.presentation.font_size
                ))
            }
            "font" | "font-name" | "font-family" => {
                if value.is_empty() {
                    return Err(
                        "font-name を指定してください: :font-name JetBrains Mono".to_string()
                    );
                }
                self.presentation.font_name = value;
                Ok(format!(
                    "font-name を {} にしました",
                    self.presentation.font_name
                ))
            }
            "theme" => {
                let theme = UiTheme::parse(&value.to_ascii_lowercase()).ok_or_else(|| {
                    "theme は dark / light / high-contrast から指定してください".to_string()
                })?;
                self.ui_theme = theme;
                Ok(format!("theme を {} にしました", self.ui_theme.label()))
            }
            "accent" | "accent-color" => {
                let accent = AccentColor::parse(&value.to_ascii_lowercase()).ok_or_else(|| {
                    "accent は blue / green / pink / yellow / red から指定してください".to_string()
                })?;
                self.accent_color = accent;
                Ok(format!(
                    "accent を {} にしました",
                    self.accent_color.label()
                ))
            }
            _ => Err(format!("未知の設定コマンドです: {}", command)),
        }
    }

    pub fn set_font_size(&mut self, size: u8) {
        self.presentation.font_size = size.clamp(8, 72);
    }

    pub fn increase_font_size(&mut self) {
        self.set_font_size(self.presentation.font_size.saturating_add(1));
        self.set_status(format!("font-size: {}", self.presentation.font_size));
    }

    pub fn decrease_font_size(&mut self) {
        self.set_font_size(self.presentation.font_size.saturating_sub(1));
        self.set_status(format!("font-size: {}", self.presentation.font_size));
    }

    pub fn cycle_font_name(&mut self) {
        let next = match self.presentation.font_name.as_str() {
            "JetBrains Mono" => "Fira Code",
            "Fira Code" => "SF Mono",
            "SF Mono" => "Hack",
            _ => "JetBrains Mono",
        };
        self.presentation.font_name = next.to_string();
        self.set_status(format!("font-name: {}", self.presentation.font_name));
    }

    pub fn cycle_theme(&mut self) {
        self.ui_theme = self.ui_theme.next();
        self.set_status(format!("theme: {}", self.ui_theme.label()));
    }

    pub fn cycle_accent(&mut self) {
        self.accent_color = self.accent_color.next();
        self.set_status(format!("accent: {}", self.accent_color.label()));
    }

    pub fn save_current_file(&mut self) -> Result<String, String> {
        let Some(path) = self.current_file.clone() else {
            return Err("保存先がありません。:write path/to/file.md を使ってください".to_string());
        };
        self.save_to_path(path)
    }

    pub fn save_to_path(&mut self, path: impl AsRef<Path>) -> Result<String, String> {
        let path = path.as_ref();
        let serialized = markdown::serialize(&self.presentation);
        fs::write(path, serialized)
            .map_err(|err| format!("保存に失敗しました: {} ({})", path.display(), err))?;
        self.current_file = Some(path.to_path_buf());
        Ok(format!("保存しました: {}", path.display()))
    }

    pub fn open_path(&mut self, path: impl AsRef<Path>) -> Result<String, String> {
        let path = path.as_ref();
        let presentation = if path.exists() {
            let source = fs::read_to_string(path)
                .map_err(|err| format!("読み込みに失敗しました: {} ({})", path.display(), err))?;
            markdown::deserialize(&source)
                .map_err(|err| format!("Markdown の解析に失敗しました: {}", err))?
        } else {
            Presentation::blank()
        };

        if matches!(
            self.running_exec.as_ref().map(|r| r.status),
            Some(ExecStatus::Running)
        ) {
            self.cancel_running_exec();
        }

        self.presentation = presentation;
        self.current_file = Some(path.to_path_buf());
        self.current_slide = 0;
        self.present_reveal_step = 0;
        self.selected_block = self
            .presentation
            .slides
            .first()
            .and_then(|slide| if slide.blocks.is_empty() { None } else { Some(0) });
        self.edit_buffer.clear();
        self.edit_cursor = 0;
        self.command_buffer.clear();
        self.command_cursor = 0;
        self.running_exec = None;
        self.next_block_id = Self::next_block_id_for(&self.presentation);

        if path.exists() {
            Ok(format!("開きました: {}", path.display()))
        } else {
            Ok(format!("新規ファイルを作成します: {}", path.display()))
        }
    }

    // ── ユーティリティ ───────────────────────────────

    pub fn total_exec_count(&self) -> usize {
        self.current_slide().exec_count()
    }

    pub fn total_signed_count(&self) -> usize {
        self.current_slide().signed_count()
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }
}
