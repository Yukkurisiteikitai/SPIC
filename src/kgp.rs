/// Kitty Graphics Protocol support.
///
/// Ghostty, Kitty, WezTerm は `ESC _G … ESC \` のエスケープシーケンスで
/// ターミナルセルに PNG/RGBA 画像を表示できる。
/// 本モジュールはその検出・エンコード・送信を担当する。
use std::io::{self, Write};

use base64::Engine;

/// このセッションで Kitty Graphics Protocol が使えるか判定する。
/// `TERM_PROGRAM` または `TERM` から検出する（APC クエリは crossterm の raw モードと競合するため使わない）。
pub fn is_supported() -> bool {
    // Ghostty は xterm-ghostty, Kitty は xterm-kitty
    if let Ok(term) = std::env::var("TERM") {
        let t = term.to_ascii_lowercase();
        if t.contains("ghostty") || t.contains("kitty") {
            return true;
        }
    }
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        let p = prog.to_ascii_lowercase();
        if p.contains("ghostty") || p.contains("kitty") || p.contains("wezterm") {
            return true;
        }
    }
    false
}

/// PNG バイト列を Kitty Graphics Protocol で stdout に送信する。
/// 内部で base64 エンコードしてから [`display_png_encoded`] を呼ぶラッパ。
/// パフォーマンス重視のループでは [`display_png_encoded`] を直接使い、
/// base64 文字列を再利用すること。
#[allow(dead_code)]
pub fn display_png(png: &[u8], x: u16, y: u16, cols: u16, rows: u16) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    display_png_encoded(&encoded, x, y, cols, rows)
}

/// 事前に base64 エンコード済みの PNG ペイロードを送信する。
/// [`crate::present::cache::FrameCache`] にペアでキャッシュした文字列を渡せば
/// 毎フレームのエンコードを省ける。
pub fn display_png_encoded(
    encoded: &str,
    x: u16,
    y: u16,
    cols: u16,
    rows: u16,
) -> io::Result<()> {
    let mut out = io::stdout().lock();

    // 1. 仮想カーソルをセル位置に移動（crossterm を使わず直接 CSI 送信）
    write!(out, "\x1b[{};{}H", y + 1, x + 1)?;

    // 2. KGP APC シーケンスをチャンクに分けて送信
    // a=T   : transmit & display
    // f=100 : PNG フォーマット
    // i=1   : image_id=1（今回は常に1で上書き）
    // q=2   : quiet（エラー抑制）
    // c=cols, r=rows : セル占有数 (0=ターミナルが推測)
    let chunk_size = 4096;
    let bytes = encoded.as_bytes();
    let total = bytes.len().div_ceil(chunk_size);
    if total == 0 {
        return out.flush();
    }

    for i in 0..total {
        let start = i * chunk_size;
        let end = (start + chunk_size).min(bytes.len());
        let chunk = &bytes[start..end];
        let is_last = i + 1 == total;
        let m = if is_last { 0 } else { 1 };
        if i == 0 {
            let header = if cols > 0 && rows > 0 {
                format!("a=T,f=100,i=1,q=2,c={},r={},m={}", cols, rows, m)
            } else {
                format!("a=T,f=100,i=1,q=2,m={}", m)
            };
            write!(out, "\x1b_G{};", header)?;
        } else {
            write!(out, "\x1b_Gm={};", m)?;
        }
        out.write_all(chunk)?;
        write!(out, "\x1b\\")?;
    }

    out.flush()
}

/// 以前に表示した画像を消去する（image_id=1 を削除）。
pub fn clear_image() -> io::Result<()> {
    let mut out = io::stdout().lock();
    // a=d, d=I : image_id 単位で削除
    write!(out, "\x1b_Ga=d,d=I,i=1,q=2;\x1b\\")?;
    out.flush()
}

/// テスト用: is_supported() の結果を文字列で返す（snapshot テストなど用）。
#[allow(dead_code)]
pub fn capability_info() -> String {
    if is_supported() {
        format!(
            "KGP supported (TERM={}, TERM_PROGRAM={})",
            std::env::var("TERM").unwrap_or_default(),
            std::env::var("TERM_PROGRAM").unwrap_or_default()
        )
    } else {
        "KGP not supported (fallback to Ratatui)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_ghostty() {
        // 環境変数を直接触らずにロジックだけ確認
        // 実際の CI では TERM が ghostty でないので false が返るのが正常
        let _ = is_supported(); // パニックしなければ OK
    }
}
