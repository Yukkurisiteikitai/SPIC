# slidecli — Claude Code 引き継ぎドキュメント

## プロジェクト概要

CLIで完結するターミナルスライド作成・発表ツール。
Rustで実装されたTUI（Ratatui）ベースのビジュアルエディタ。

**コアコンセプト**
- Markdownを保存形式として使うが、内部モデルは `Vec<Block>` — Markdownは出力形式
- 実行可能コードブロック（execブロック）はマシンローカルの秘密鍵署名がないと動かない（Deny by default）
- AI（Claude/Codex）は署名判断の補助ツール。AIの判定を自動実行条件にはしない
- CLIに完結。外部ブラウザ・Electron不要

---

## 現在の実装状態

### ✅ 完成済み

- Ratatuiによるビジュアルエディタ（3ペイン: サイドバー・キャンバス・ツールバー）
- ブロック型データモデル（Heading / Text / Code / Exec / OutputPlaceholder / Separator）
- ノーマルモード / テキスト編集モード / ブロック追加パレット
- execブロックの操作フロー（編集 `e` → 署名 `s` → 実行確認 `Space` → 実行 `y`）
- プレゼンモード（全画面、`p`で入り`Esc`で戻る）
- exec実行確認ダイアログ（オーバーレイ）
- execコマンドの実際の実行（`std::process::Command`）+ stdout をOutputPlaceholderに表示
- モック署名（本物のEd25519はまだ）

### ❌ 未実装（次にやること）

優先順に並べてある。

#### 1. Ed25519署名エンジン（最優先）

`src/signing.rs` を新規作成して以下を実装する。

```rust
// 実装すべきAPI
pub fn generate_keypair() -> Result<()>          // ~/.slidecli/keys/ に鍵ペア生成
pub fn sign_block(content: &str) -> Result<String>   // "sig:ed25519:<hex>" を返す
pub fn verify_block(content: &str, sig: &str) -> bool // 署名検証
```

- 秘密鍵: `~/.slidecli/keys/id_ed25519`（バイナリ）
- 公開鍵: `~/.slidecli/keys/id_ed25519.pub`
- 使用クレート: `ring = "0.17"` または `ed25519-dalek = "2"`
- 現在 `app.rs` の `sign_selected()` にモック実装あり → 本物に置き換える
- Cargo.toml の Rustバージョン制約注意（現在 rustc 1.75.0）

#### 2. AI審査フロー（input.rs の `'a'` キー）

`src/ai_review.rs` を新規作成。

```rust
pub async fn review_exec_block(content: &str) -> Result<AiVerdict>

pub struct AiVerdict {
    pub risk: RiskLevel,   // Low / Medium / High
    pub reason: String,
    pub recommend_exec: bool,
}
```

- `ANTHROPIC_API_KEY` 環境変数があれば Claude API を呼び出す
- `OPENAI_API_KEY` があれば OpenAI API にフォールバック
- どちらもなければ「AI審査スキップ」メッセージを表示
- **AIの判定を自動実行の条件にしてはいけない**。ユーザーへの参考情報として表示するだけ
- コードを実行させない。テキストとして渡して静的解析させる
- システムプロンプト例:
  ```
  以下のシェルコマンドを静的解析してください。実行はしないでください。
  危険なパターン（ファイル削除、外部通信、認証情報アクセス、間接実行）を検出し、
  危険度[低/中/高]と理由を簡潔に返してください。JSON形式で返答。
  ```

#### 3. Markdownシリアライザ / デシリアライザ

`src/markdown.rs` を新規作成。

```rust
pub fn serialize(presentation: &Presentation) -> String
pub fn deserialize(src: &str) -> Result<Presentation>
```

- `---` でスライド区切り
- execブロックのアノテーション形式: ` ```lang exec sig:ed25519:<hex> `
- 未署名execブロック: ` ```lang exec `
- OutputPlaceholderは `<!-- output -->` コメントで表現
- ラウンドトリップ保証: serialize → deserialize → serialize が同一出力になること

#### 4. Wizard（初回起動時）

- フォント選択（Nerd Fonts系のリストから選ぶ）
- カラーテーマ選択（dark / light / custom）
- 設定を `~/.slidecli/config.toml` に保存
- p10k `configure` に近いUX — 選択肢を見せながら即時プレビュー更新

#### 5. ファイルの保存・読み込み

- `Ctrl+S` → Markdownとして保存（現在スタブ）
- 起動引数でファイルを受け取る: `slidecli edit presentation.md`

---

## アーキテクチャ

```
src/
├── main.rs          イベントループ、モード別描画ディスパッチ
├── lib.rs           公開エクスポート（snapshotテスト用）
├── model.rs         Block / Slide / Presentation のデータ構造
├── app.rs           状態管理・ビジネスロジック（ナビ・編集・exec実行）
├── ui.rs            Ratatui描画（draw / draw_present / draw_exec_confirm）
├── input.rs         キーハンドラー（モード別）
├── bin/
│   └── snapshot.rs  TestBackendでUIスナップショット出力（開発用）
│
│   ── 以下は未作成 ──
├── signing.rs       Ed25519署名エンジン（TODO）
├── ai_review.rs     AI審査フロー（TODO）
└── markdown.rs      Markdown serialize/deserialize（TODO）
```

---

## モデル詳細

```rust
// src/model.rs

pub enum BlockKind {
    Heading { level: u8 },
    Text,
    Code { lang: String },
    Exec { lang: String, signature: Option<String> },  // Noneなら未署名
    OutputPlaceholder,
    Separator,
}

pub struct Block {
    pub id: u64,
    pub kind: BlockKind,
    pub content: String,
}

pub struct Slide {
    pub title: String,   // サイドバー表示用（Markdownには保存しない）
    pub blocks: Vec<Block>,
}

pub struct Presentation {
    pub slides: Vec<Slide>,
    pub font_name: String,
    pub font_size: u8,
}
```

---

## モード一覧

```rust
pub enum AppMode {
    Normal,        // ナビゲーション（h/l/j/k）
    EditingBlock,  // テキスト編集中（e で入る、Esc/Enter で確定）
    BlockPicker,   // ブロック追加パレット（n で開く、1-6 で選択）
    ExecConfirm,   // exec実行確認ダイアログ（Space で開く、y/n で操作）
    Present,       // プレゼン全画面（p で入る、Esc で戻る）
}
```

---

## キーバインド（現在の実装）

### ノーマルモード

| キー | 動作 |
|------|------|
| `h` / `l` | スライド移動 |
| `j` / `k` | ブロック選択 |
| `e` / `Enter` | ブロック編集開始 |
| `n` | ブロック追加パレット |
| `K` / `J` | ブロックを上/下へ移動 |
| `N` | スライド追加 |
| `Space` | execブロック実行確認（署名済みのみ）|
| `s` | 選択中execブロックに署名 |
| `a` | AI審査（スタブ） |
| `p` | プレゼンモードへ |
| `1`-`9` | スライドジャンプ |
| `Esc` | ステータスメッセージクリア |
| `q` | 終了 |

### 編集モード

| キー | 動作 |
|------|------|
| `Esc` | 編集キャンセル |
| `Enter` | 編集確定 |
| `Shift+Enter` | 改行挿入 |
| `Backspace` | 文字削除 |
| `←` / `→` | カーソル移動 |

### プレゼンモード

| キー | 動作 |
|------|------|
| `h` / `l` / `Space` | スライド移動 |
| `Esc` / `q` | 編集に戻る |

### exec確認ダイアログ

| キー | 動作 |
|------|------|
| `y` / `Enter` | 実行 |
| `n` / `Esc` | キャンセル |

---

## セキュリティ設計方針（重要）

### 鍵モデル
- **マシンローカル** — 鍵は `~/.slidecli/keys/` に置き、他マシンへの移植機能は作らない
- 別マシンでは再署名（＝再確認）が必要。これは意図した動作
- 鍵の共有・チーム署名機能は作らない（「人への信頼」ではなく「このマシンで確認したか」）

### execブロックのライフサイクル
```
未署名 → [s で署名] → 署名済 → [Space] → 確認ダイアログ → [y] → 実行
                                                         ↓
                                              [a でAI審査] → 参考情報表示 → ユーザーが判断
```

### 絶対に守ること
- 署名なしのexecブロックは **絶対に自動実行しない**
- AIの判定を自動実行の条件にしない（AIはあくまで参考情報）
- コマンド内容を変えたら署名を自動リセット（`commit_edit()` で実装済み）

---

## 依存クレート（Cargo.toml）

```toml
ratatui = "=0.24.0"
crossterm = "=0.26.0"
serde = { version = "=1.0.193", features = ["derive"] }
anyhow = "=1.0.75"
```

**注意**: rustc 1.75.0 環境のため、新しいクレートを追加するときはバージョン制約に注意。
`cargo fetch` 後に `edition2024` エラーが出た場合は古いバージョンを指定する。
`unicode-segmentation` は `=1.11.0` に固定してある（Cargo.lock参照）。

---

## カラーパレット（ui.rs）

モックスクリーンショットに合わせたダークテーマ。変更時は `ui.rs` 冒頭の定数を修正。

```rust
const BG_BASE:      Color = Rgb(26, 26, 26)   // メイン背景
const BG_SIDEBAR:   Color = Rgb(22, 22, 22)   // サイドバー
const FG_ACCENT:    Color = Rgb(74, 158, 255) // 青（選択中・モード表示）
const FG_EXEC:      Color = Rgb(106, 176, 76) // 緑（execブロック）
const FG_WARN:      Color = Rgb(255, 107, 107)// 赤（未署名・警告）
```

---

## 開発用コマンド

```bash
cargo run                      # エディタ起動（default-run = "slidecli"）
cargo run --bin snapshot       # UIスナップショットをstdoutに出力（非インタラクティブ確認用）
cargo build --release          # リリースビルド
```

---

## 既知の問題・TODO

- [ ] ステータスメッセージが時間経過で消えない（フレームカウンターかタイムスタンプで消す）
- [ ] サイドバーのスライドタイトルがH1ブロックから自動更新されない
- [ ] プレゼンモードで画像ブロックが未対応
- [ ] ツールバーが画面幅に対してオーバーフローすることがある
- [ ] 日本語入力（IME）はターミナルのrawモードとの相性問題あり（要調査）
