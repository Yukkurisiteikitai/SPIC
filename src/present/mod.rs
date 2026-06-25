//! プレゼンモード（KGP=Kitty Graphics Protocol）の最適化サブシステム。
//!
//! - [`cache`]  : フォント / グリフ / RGBA キャンバス / PNG+base64 のキャッシュ層
//! - [`fingerprint`] : 1 フレームを一意に識別するハッシュ
//! - [`state`]  : `App` に持たせる派生状態（dirty flag, 前回値）
//! - [`runtime`] : プレゼン中 1 フレームのライフサイクル統合エントリ
//!
//! 既存の [`crate::render`] と [`crate::ui`] の描画ロジックには手を入れず、
//! 「外側からキャッシュを注入し、再描画判定をフックする」設計。

pub mod cache;
pub mod fingerprint;
pub mod runtime;
pub mod state;

#[allow(unused_imports)]
pub use cache::{CachedFrame, CachedGlyph, CanvasPool, FrameCache, GlyphCache, load_font_bytes};
#[allow(unused_imports)]
pub use fingerprint::FrameFingerprint;
#[allow(unused_imports)]
pub use runtime::{PresentCaches, tick_present_kgp};
pub use state::PresentState;
