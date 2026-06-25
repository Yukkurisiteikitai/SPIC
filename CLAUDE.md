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
- ノーマルモード / テキスト編集モード / ブロック追加パレット / 設定画面 / コマンド入力
- execブロックの操作フロー（編集 `e` → 署名 `s` → 実行確認 `Space` → 実行 `y`）
- プレゼンモード（全画面、`p`で入り`Esc`で戻る）
- exec実行確認ダイアログ（オーバーレイ、編集中・プレゼン中の両方に対応）
- execコマンドの実際の実行（`std::process::Command`）+ stdout/stderr をOutputPlaceholderに表示
- モック署名（本物のEd25519はまだ）
- UI設定の永続化（`.slidecli-settings.json`）: font_name / font_size / ui_theme / accent_color
- **KGP（Kitty Graphics Protocol）プレゼンレンダラー** — Ghostty/Kitty/WezTerm で `p` を押すと
  スライドをピクセルレンダリングした画像として表示。font_size に応じて文字サイズが実際に変わる。

### ❌ 未実装（次にやること）

優先順に並べてある。

#### 1. Ed25519署名エンジン（最優先）

`src/signing.rs` を新規作成して以下を実装する。

```rust
pub fn generate_keypair() -> Result<()>           // ~/.slidecli/keys/ に鍵ペア生成
pub fn sign_block(content: &str) -> Result<String>    // "sig:ed25519:<hex>" を返す
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

#### 4. ファイルの保存・読み込み

- `Ctrl+S` → Markdownとして保存（現在スタブ）
- 起動引数でファイルを受け取る: `slidecli edit presentation.md`

#### 5. KGP セルサイズの自動検出

現在は `SLIDECLI_CELL_W` / `SLIDECLI_CELL_H` 環境変数（デフォルト 8×16px）。
xterm の `XTWINOPS 14` (CSI 14 t) で実際のピクセルサイズを問い合わせることができるが、
Ghostty の対応状況を確認してから実装する。

---

## アーキテクチャ

```
src/
├── main.rs       イベントループ、モード別描画ディスパッチ、KGP/Ratatui 切替
├── lib.rs        公開エクスポート（テスト用）
├── model.rs      Block / Slide / Presentation のデータ構造
├── app.rs        状態管理・ビジネスロジック（ナビ・編集・exec実行・設定）
├── ui.rs         Ratatui描画（編集モード全画面、プレゼンフォールバック）
├── input.rs      キーハンドラー（モード別）
├── config.rs     UiSettings の JSON 読み書き（.slidecli-settings.json）
├── kgp.rs        Kitty Graphics Protocol 検出・送信（display_png / clear_image）
├── render.rs     スライド→PNG レンダラー（fontdue + image クレート）
└── bin/
    ├── snapshot.rs   TestBackendでUIスナップショット出力（開発用）
    └── smoke_exec.rs exec フローの非インタラクティブテスト

── 以下は未作成 ──
├── signing.rs    Ed25519署名エンジン（TODO）
├── ai_review.rs  AI審査フロー（TODO）
└── markdown.rs   Markdown serialize/deserialize（TODO）
```

---

## KGP プレゼンレンダラー（render.rs / kgp.rs）

### 動作原理

`p` キーでプレゼンモードへ入ると `main.rs` が KGP 対応端末を検出し、
Ratatui の代わりに画像パスで描画する。

```
Presentation → render_slide_to_png()      ← src/render.rs
    ↓ fontdue で Hiragino Sans GB をラスタライズ
    ↓ image クレートで RGBA バッファ → PNG エンコード
    ↓
kgp::display_png(png, x=0, y=0, cols, rows)  ← src/kgp.rs
    ↓ base64 エンコード → ESC _G ... ESC \ で端末に送信
    ↓ Ghostty が PNG をピクセル描画
```

### KGP 端末判定（kgp::is_supported）

`TERM=xterm-ghostty` または `TERM_PROGRAM=ghostty/kitty/wezterm` で判定。
対応端末以外は自動的に Ratatui フォールバック。

### font_size の効果

| font_size | 見出し px | 本文 px |
|-----------|-----------|---------|
| 14        | ~34px     | ~17px   |
| 20        | ~48px     | ~24px   |
| 27        | ~65px     | ~32px   |

`+`/`-` キーで変更するたびに再レンダリングされる。

### フォント優先順位（render.rs: FONT_CANDIDATES）

1. `/System/Library/Fonts/Hiragino Sans GB.ttc` — macOS、ASCII+CJK対応
2. `/System/Library/Fonts/Menlo.ttc` — macOS ASCII fallback
3. `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc` — Linux
4. `/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf` — Linux ASCII

フォントが1つも見つからない場合は Ratatui フォールバックへ自動切替。

### セルサイズ調整

KGP はピクセル画像を端末セルにマッピングする。
端末フォントサイズに合わせて調整が必要な場合:

```bash
SLIDECLI_CELL_W=10 SLIDECLI_CELL_H=20 cargo run
```

デフォルト: `cell_w=8, cell_h=16`。Ghostty デフォルトフォント(13pt)では `8×19` 前後が適切。

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
    pub font_size: u8,   // 8〜72。KGP パスではピクセルサイズに直接影響
}
```

---

## モード一覧

```rust
pub enum AppMode {
    Normal,              // ナビゲーション（h/l/j/k）
    EditingBlock,        // テキスト編集中（e で入る、Esc/Enter で確定）
    BlockPicker,         // ブロック追加パレット（n で開く、1-6 で選択）
    ExecConfirm,         // exec実行確認ダイアログ（Space で開く、y/n で操作）
    Settings,            // 設定画面（Ctrl+Q で開く）
    CommandInput,        // : コマンド入力
    PresentExecConfirm,  // プレゼン中 exec 実行確認
    Present,             // プレゼン全画面（p で入る、Esc で戻る）
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
| `c` | 実行中 exec をキャンセル |
| `[` / `]` | 出力スクロール上/下 |
| `a` | AI審査（スタブ） |
| `p` | プレゼンモードへ |
| `Ctrl+Q` | 設定画面へ |
| `:` | コマンド入力 |
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
| `+` / `-` | font_size 変更（再レンダリング） |
| `j` / `k` | ブロック選択（exec 操作用） |
| `s` | 選択中 exec に署名 |
| `Space` / `Enter` | exec実行確認（exec選択時）/ 次スライド |
| `c` | 実行中 exec キャンセル |
| `:` | コマンド入力 |
| `Esc` / `q` | 編集に戻る |

### 設定画面

| キー | 動作 |
|------|------|
| `+` / `-` | font_size 変更 |
| `f` | font_name サイクル |
| `t` | テーマ切替（dark/light/high-contrast） |
| `a` | アクセントカラー切替 |
| `:` | コマンド入力（`:font-size 20` 等） |
| `Esc` / `q` | 戻る |

### コマンド一覧（`:` 入力）

| コマンド | 例 |
|----------|-----|
| `font-size <n>` | `:font-size 24` |
| `font-name <name>` | `:font-name Fira Code` |
| `theme <dark\|light\|high-contrast>` | `:theme dark` |
| `accent <color>` | `:accent green` |

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
ratatui    = "=0.24.0"
crossterm  = "=0.26.0"
serde      = { version = "=1.0.193", features = ["derive"] }
serde_json = "=1.0.108"
anyhow     = "=1.0.75"
fontdue    = "=0.8.0"           # フォントラスタライザ（KGP レンダラー用）
image      = { version = "=0.24.7", default-features = false, features = ["png"] }
base64     = "=0.21.5"          # KGP ペイロードエンコード
```

**注意**: rustc 1.75.0 環境のため、新しいクレートを追加するときはバージョン固定で。
`cargo fetch` 後に `edition2024` エラーが出た場合は古いバージョンを指定する。

---

## カラーパレット（ui.rs / render.rs）

エディタ（Ratatui）と KGP レンダラー（render.rs）でカラーを合わせてある。
変更するときは両方を修正する。

```rust
// ui.rs
const BG_BASE:    Color = Rgb(26, 26, 26)    // メイン背景
const BG_PRESENT: Color = Rgb(18, 18, 18)    // プレゼン背景
const FG_PRIMARY: Color = Rgb(224, 224, 224) // メインテキスト
const FG_ACCENT:  Color = Rgb(74, 158, 255)  // 青（選択中・モード表示）
const FG_EXEC:    Color = Rgb(106, 176, 76)  // 緑（execブロック）
const FG_WARN:    Color = Rgb(255, 107, 107) // 赤（未署名・警告）

// render.rs（同値を [u8;4] で定義）
const BG:         [u8; 4] = [18, 18, 18, 255]
const FG_HEADING: [u8; 4] = [224, 224, 224, 255]
const FG_ACCENT:  [u8; 4] = [74, 158, 255, 255]
```

---

## 開発用コマンド

```bash
cargo run                              # エディタ起動
cargo run --bin snapshot               # UIスナップショット stdout 出力（非インタラクティブ）
cargo test                             # 全テスト実行（8件）
cargo test render_slide_to_png -- --ignored --nocapture  # PNG書き出しテスト
cargo build --release                  # リリースビルド

# KGP セルサイズを調整して起動
SLIDECLI_CELL_W=10 SLIDECLI_CELL_H=20 cargo run
```

---

## テスト一覧

| テスト | ファイル | 内容 |
|--------|---------|------|
| `roundtrip_settings_json` | config.rs | UiSettings の JSON シリアライズ往復 |
| `editor_layout_is_stable_across_font_sizes` | ui.rs | 編集画面は font_size に影響されない |
| `present_ascii_heading_grows_with_font_size` | ui.rs | ASCII 見出しが font_size で拡大する |
| `present_japanese_heading_does_not_panic` | ui.rs | 日本語見出しがパニックしない |
| `test_is_supported_ghostty` | kgp.rs | KGP 検出ロジックがパニックしない |
| `font_load_attempt_does_not_panic` | render.rs | フォント未発見でも None を返す |
| `wrap_text_works` | render.rs | 折り返しロジック |
| `render_slide_to_png_generates_bytes` | render.rs | PNG が正しく生成される |

---

## 既知の問題・TODO

- [ ] ステータスメッセージが時間経過で消えない（フレームカウンターかタイムスタンプで消す）
- [ ] サイドバーのスライドタイトルがH1ブロックから自動更新されない
- [ ] KGP セルサイズの自動検出（現在は環境変数手動設定）
- [ ] プレゼン中 exec 実行確認ダイアログ（PresentExecConfirm）を KGP 上に重ねる方法の検討
- [ ] Ratatui プレゼンモードの bigtext 見出し（ASCII のみ対応、日本語は行リピート）
- [ ] ツールバーが画面幅に対してオーバーフローすることがある
- [ ] 日本語入力（IME）はターミナルの raw モードとの相性問題あり（要調査）
