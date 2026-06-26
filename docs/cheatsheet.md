# slidecli Cheat Sheet

## 起動

```bash
cargo run
cargo run -- presentation.md
cargo run -- edit presentation.md
slidecli presentation.md
```

- `cargo run` は組み込みデモを開く
- `cargo run -- file.md` は既存 Markdown を開く
- ファイルが無ければ、そのパスで新規作成前提の空プレゼンを開く

## 保存

- `Ctrl+S`: 現在のファイルに保存
- `:write other.md`: 別名保存
- `:open another.md`: 別ファイルを開く

## スライド区切り

```md
# 1枚目
本文

---

# 2枚目
本文
```

- `---` がスライド区切り

## 対応している基本ブロック

### 見出し

```md
# H1
## H2
### H3
```

### テキスト

```md
普通の段落

> 引用

- 箇条書き
```

- いまは細かい Markdown 構文を全部別ブロックとして解釈するわけではない
- ふつうの本文テキストとして扱う部分がある

### コードブロック

````md
```bash
echo hello
```
````

## 段階表示

```md
最初に見せる文章
<!-- slidecli:next:start -->
次で出したい文章
<!-- slidecli:next:end -->
```

- プレゼン中の `Space` / `Enter` / `l` / `Right` で次の段階へ進む
- すべて表示し終わったら次スライドへ進む
- `h` / `Left` / `Backspace` で戻る

## exec ブロック

````md
<!-- slidecli:block type=exec lang="bash" sig="sig:ed25519:abc123" -->
```bash
echo hello
```
````

- `type=exec` で実行可能ブロックになる
- `lang` は表示用
- `sig` は署名文字列
- 現状の署名はアプリ内の開発用モック署名

## 出力プレースホルダ

````md
<!-- slidecli:block type=output -->
```text
ここに出力が入る
```
````

重要:

- 実行結果を表示したいなら、`output` ブロックは **対象の `exec` ブロックの直後** に置く
- 直後でない `output` ブロックには現在は紐付かない
- `output` の初期内容は空でもよい

### exec + output の最小例

````md
# Demo

<!-- slidecli:block type=exec lang="bash" sig="sig:ed25519:abc123" -->
```bash
echo hello
```

<!-- slidecli:block type=output -->
```text
```
````

## プレゼン中の主な操作

- `p`: プレゼンモードへ
- `Esc` / `q`: プレゼン終了
- `j` / `k`: ブロック選択
- `Space` / `Enter`: 段階表示を進める、または exec 実行確認
- `h` / `l`: 前後に進む
- `s`: exec ブロックに署名
- `c`: 実行中の exec をキャンセル

## 編集中の主な操作

- `e` / `Enter`: ブロック編集
- `n`: ブロック追加パレット
- `N`: スライド追加
- `:`: コマンド入力
- `Ctrl+Q`: 設定

## 現状の制約

- まだ汎用 Markdown スライドエディタとしては途中段階
- `exec` の出力先は「直後の output ブロック」だけ
- 画像や高度な Markdown 構文は未対応
- 保存形式は Markdown ベースだが、slidecli 固有情報は HTML コメントで保持する
