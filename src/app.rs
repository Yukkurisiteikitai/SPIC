use crate::model::{Presentation, BlockKind, Block};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,           // ナビゲーション（ブロック選択・スライド移動）
    EditingBlock,     // テキスト編集中
    BlockPicker,      // ブロック追加パレット
    ExecConfirm,      // exec実行確認オーバーレイ
    Present,          // プレゼンモード（全画面）
}

pub struct App {
    pub presentation: Presentation,
    pub current_slide: usize,
    pub selected_block: Option<usize>,
    pub mode: AppMode,
    pub edit_buffer: String,
    pub edit_cursor: usize,
    pub status_message: Option<String>,
    pub next_block_id: u64,
    // execブロック実行結果（block_id -> stdout）
    pub exec_outputs: std::collections::HashMap<u64, String>,
}

impl App {
    pub fn new(presentation: Presentation) -> Self {
        let next_id = presentation.slides.iter()
            .flat_map(|s| s.blocks.iter())
            .map(|b| b.id)
            .max()
            .unwrap_or(0) + 1;

        Self {
            presentation,
            current_slide: 0,
            selected_block: Some(0),
            mode: AppMode::Normal,
            edit_buffer: String::new(),
            edit_cursor: 0,
            status_message: None,
            next_block_id: next_id,
            exec_outputs: std::collections::HashMap::new(),
        }
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
        self.selected_block.and_then(|i| self.current_slide().blocks.get(i))
    }

    pub fn is_exec_selected(&self) -> bool {
        self.selected_block_ref().map(|b| b.is_exec()).unwrap_or(false)
    }

    pub fn is_signed_selected(&self) -> bool {
        self.selected_block_ref().map(|b| b.is_signed()).unwrap_or(false)
    }

    // ── ナビゲーション ──────────────────────────────

    pub fn prev_slide(&mut self) {
        if self.current_slide > 0 {
            self.current_slide -= 1;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn next_slide(&mut self) {
        if self.current_slide + 1 < self.presentation.slides.len() {
            self.current_slide += 1;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn go_to_slide(&mut self, idx: usize) {
        if idx < self.presentation.slides.len() {
            self.current_slide = idx;
            let len = self.current_slide().blocks.len();
            self.selected_block = if len == 0 { None } else { Some(0) };
        }
    }

    pub fn prev_block(&mut self) {
        if let Some(sel) = self.selected_block {
            if sel > 0 { self.selected_block = Some(sel - 1); }
        }
    }

    pub fn next_block(&mut self) {
        let len = self.current_slide().blocks.len();
        match self.selected_block {
            Some(sel) if sel + 1 < len => self.selected_block = Some(sel + 1),
            None if len > 0            => self.selected_block = Some(0),
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
            let buf = self.edit_buffer.clone();
            let slide = self.current_slide_mut();
            let content_changed = slide.blocks[sel].content != buf;
            // コマンドを変えたらexecブロックの署名をリセット
            if content_changed {
                if let BlockKind::Exec { ref mut signature, .. } = slide.blocks[sel].kind {
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
                self.set_status("未署名のexecブロックです。's'で署名、'a'でAI審査してください".to_string());
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
                if let BlockKind::Exec { ref mut signature, .. } = slide.blocks[sel].kind {
                    *signature = Some(hash);
                }
                self.set_status("署名しました（開発用モック署名）".to_string());
            } else {
                self.set_status("execブロックを選択してください".to_string());
            }
        }
    }

    /// exec実行（署名済みのみ）
    pub fn run_exec_selected(&mut self) {
        if let Some(sel) = self.selected_block {
            if !self.is_signed_selected() {
                self.set_status("署名されていません。's'で署名してください".to_string());
                self.mode = AppMode::Normal;
                return;
            }
            let block = &self.current_slide().blocks[sel];
            let block_id = block.id;
            let cmd = block.content.clone();

            // シェルコマンドを実行
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output();

            let result = match output {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    if stdout.is_empty() && !stderr.is_empty() {
                        format!("[stderr]\n{}", stderr.trim())
                    } else if stdout.is_empty() {
                        "(出力なし)".to_string()
                    } else {
                        stdout.trim().to_string()
                    }
                }
                Err(e) => format!("[実行エラー] {}", e),
            };

            // 出力をOutputPlaceholderブロックに書き込む
            self.exec_outputs.insert(block_id, result.clone());

            // 直後のOutputPlaceholderを探して更新
            let slide = self.current_slide_mut();
            if let Some(placeholder) = slide.blocks.get_mut(sel + 1) {
                if matches!(placeholder.kind, BlockKind::OutputPlaceholder) {
                    placeholder.content = result;
                }
            }

            self.set_status("実行完了".to_string());
        }
        self.mode = AppMode::Normal;
    }

    // ── ブロック追加・削除・移動 ─────────────────────

    pub fn add_block(&mut self, kind: BlockKind) {
        let id = self.next_block_id;
        self.next_block_id += 1;
        let block = Block::new(id, kind, "");
        let insert_pos = self.selected_block.map(|s| s + 1)
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
        self.mode = AppMode::Present;
        self.current_slide = 0;
    }

    pub fn exit_present(&mut self) {
        self.mode = AppMode::Normal;
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
