use std::time::Instant;

use crate::present::fingerprint::FrameFingerprint;

/// プレゼンレンダリングの派生状態。`App` のフィールドとして埋め込む。
///
/// `App` の編集ロジックは一切触らず、`runtime::tick_present_kgp` と
/// 入力ハンドラから dirty flag を立てる用途にのみ使う。
#[derive(Debug, Default)]
pub struct PresentState {
    /// 明示的な再描画要求（モード遷移・exec 出力差分・リサイズ等で立てる）
    pub needs_redraw: bool,
    /// 直近 KGP に送信したフレームの fingerprint。`None` なら未表示。
    pub last_displayed_fingerprint: Option<FrameFingerprint>,
    /// 直近 KGP に送信した時刻（exec 実行中の経過時間表示更新トリガに使う）
    pub last_render_at: Option<Instant>,
    /// 直近の端末サイズ（リサイズ検出のフォールバック）
    pub last_term_size: Option<(u16, u16)>,
}

impl PresentState {
    pub fn new() -> Self {
        Self::default()
    }

    /// プレゼン突入／退出時にリセットする。
    pub fn reset(&mut self) {
        self.needs_redraw = true;
        self.last_displayed_fingerprint = None;
        self.last_render_at = None;
        self.last_term_size = None;
    }

    /// 描画が完了したことを記録する。
    pub fn mark_rendered(&mut self, fp: FrameFingerprint, term_size: (u16, u16)) {
        self.needs_redraw = false;
        self.last_displayed_fingerprint = Some(fp);
        self.last_render_at = Some(Instant::now());
        self.last_term_size = Some(term_size);
    }

    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }
}
