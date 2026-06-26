use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockKind {
    Heading { level: u8 },   // H1〜H3
    Text,                     // 段落・箇条書き
    Code { lang: String },    // 表示のみコードブロック
    Exec { lang: String, signature: Option<String> }, // 実行可能ブロック
    OutputPlaceholder,        // exec結果の表示エリア
    Separator,                // スライド内区切り
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: u64,
    pub kind: BlockKind,
    pub content: String,
}

impl Block {
    pub fn new(id: u64, kind: BlockKind, content: impl Into<String>) -> Self {
        Self { id, kind, content: content.into() }
    }

    pub fn label(&self) -> &'static str {
        match &self.kind {
            BlockKind::Heading { .. }        => "見出し",
            BlockKind::Text                  => "テキスト",
            BlockKind::Code { .. }           => "コード",
            BlockKind::Exec { signature, .. } => {
                if signature.is_some() { "exec · 署名済" } else { "exec · 未署名" }
            },
            BlockKind::OutputPlaceholder     => "出力プレースホルダ",
            BlockKind::Separator             => "区切り",
        }
    }

    pub fn is_exec(&self) -> bool {
        matches!(self.kind, BlockKind::Exec { .. })
    }

    pub fn is_signed(&self) -> bool {
        matches!(&self.kind, BlockKind::Exec { signature: Some(_), .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub title: String,       // サイドバー表示用
    pub blocks: Vec<Block>,
}

impl Slide {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), blocks: Vec::new() }
    }

    pub fn exec_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.is_exec()).count()
    }

    pub fn signed_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.is_signed()).count()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Presentation {
    pub slides: Vec<Slide>,
    pub font_name: String,
    pub font_size: u8,
}

impl Presentation {
    pub fn blank() -> Self {
        Self {
            slides: vec![Slide::new("Slide 1")],
            font_name: "JetBrains Mono".to_string(),
            font_size: 14,
        }
    }

    pub fn demo() -> Self {
        let mut p = Self {
            slides: Vec::new(),
            font_name: "JetBrains Mono".to_string(),
            font_size: 14,
        };

        // スライド1: タイトル
        let mut s1 = Slide::new("タイトル");
        s1.blocks.push(Block::new(1, BlockKind::Heading { level: 1 }, "slidecli デモ"));
        s1.blocks.push(Block::new(2, BlockKind::Text, "CLIで完結するスライド作成ツール"));
        p.slides.push(s1);

        // スライド2: 概要
        let mut s2 = Slide::new("概要");
        s2.blocks.push(Block::new(3, BlockKind::Heading { level: 1 }, "概要"));
        s2.blocks.push(Block::new(4, BlockKind::Text, "• Markdown互換の内部ブロックモデル\n• ローカル秘密鍵による実行ブロック署名\n• AI審査（Claude / Codex）連携"));
        p.slides.push(s2);

        // スライド3: デモ実行
        let mut s3 = Slide::new("デモ実行");
        s3.blocks.push(Block::new(5, BlockKind::Heading { level: 1 }, "Cargo テスト実行デモ"));
        s3.blocks.push(Block::new(6, BlockKind::Text, "テストスイートを実際に動かしてみます"));
        s3.blocks.push(Block::new(7, BlockKind::Exec {
            lang: "rust".to_string(),
            signature: Some("sig:ed25519:3f8a2c...".to_string()),
        }, "cargo test --release 2>&1"));
        s3.blocks.push(Block::new(8, BlockKind::OutputPlaceholder, ""));
        p.slides.push(s3);

        // スライド4: 結果
        let mut s4 = Slide::new("結果");
        s4.blocks.push(Block::new(9, BlockKind::Heading { level: 1 }, "実行結果"));
        s4.blocks.push(Block::new(10, BlockKind::Text, "テスト結果がここに表示されます"));
        p.slides.push(s4);

        // スライド5: まとめ
        let mut s5 = Slide::new("まとめ");
        s5.blocks.push(Block::new(11, BlockKind::Heading { level: 2 }, "まとめ"));
        s5.blocks.push(Block::new(12, BlockKind::Text, "• CLI完結のビジュアルエディタ\n• デフォルト安全な実行モデル\n• AI支援の署名フロー"));
        p.slides.push(s5);

        p
    }
}
