//! プレゼンレンダリングのキャッシュ層。
//!
//! - [`load_font_bytes`] : フォントファイル探索（旧 `render.rs` から移動）
//! - [`font`] : `fontdue::Font` のプロセス内シングルトン
//! - [`GlyphCache`] : `(char, font_px)` キーのラスタライズ結果キャッシュ（FIFO エビクション）
//! - [`CanvasPool`] : RGBA バッファの再利用プール
//! - [`FrameCache`] : 直前 1 枚の PNG+base64 を保持

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::present::fingerprint::FrameFingerprint;

// ──────────────────────────────────────────────────────────────────────────────
// フォントロード（旧 render.rs から移動。挙動同一）
// ──────────────────────────────────────────────────────────────────────────────

const FONT_CANDIDATES: &[&str] = &[
    // macOS — Hiragino は ASCII + CJK を全て含む
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    // macOS モノスペース（ASCII fallback）
    "/System/Library/Fonts/Menlo.ttc",
    // Linux — Noto Sans CJK
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
];

static FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();
static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();

pub fn load_font_bytes() -> Option<&'static Vec<u8>> {
    FONT_DATA
        .get_or_init(|| {
            for path in FONT_CANDIDATES {
                if Path::new(path).exists() {
                    if let Ok(bytes) = std::fs::read(path) {
                        return Some(bytes);
                    }
                }
            }
            None
        })
        .as_ref()
}

/// プロセス内で 1 度だけ `fontdue::Font` を構築する。
/// rasterize 時の `px` 引数で実サイズが決まるため、初期 scale は代表値で良い。
pub fn font() -> Option<&'static fontdue::Font> {
    FONT.get_or_init(|| {
        let bytes = load_font_bytes()?;
        fontdue::Font::from_bytes(
            bytes.as_slice(),
            fontdue::FontSettings {
                collection_index: 0,
                scale: 48.0,
                load_substitutions: true,
            },
        )
        .ok()
    })
    .as_ref()
}

// ──────────────────────────────────────────────────────────────────────────────
// グリフキャッシュ
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CachedGlyph {
    pub metrics: fontdue::Metrics,
    pub bitmap: Vec<u8>,
}

pub struct GlyphCache {
    entries: HashMap<(char, u32), Rc<CachedGlyph>>,
    order: VecDeque<(char, u32)>,
    max_entries: usize,
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::with_capacity(4096)
    }
}

impl GlyphCache {
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries.min(256)),
            order: VecDeque::with_capacity(max_entries.min(256)),
            max_entries: max_entries.max(1),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `font.rasterize(ch, px)` の結果をキャッシュ経由で返す。
    /// キーは `(ch, (px * 100.0) as u32)` で 0.01px 精度に丸める。
    pub fn rasterize(&mut self, font: &fontdue::Font, ch: char, px: f32) -> Rc<CachedGlyph> {
        let key = (ch, (px * 100.0) as u32);
        if let Some(g) = self.entries.get(&key) {
            return g.clone();
        }
        let (metrics, bitmap) = font.rasterize(ch, px);
        let g = Rc::new(CachedGlyph { metrics, bitmap });
        self.insert(key, g.clone());
        g
    }

    fn insert(&mut self, key: (char, u32), g: Rc<CachedGlyph>) {
        if self.entries.len() >= self.max_entries {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
        self.entries.insert(key, g);
        self.order.push_back(key);
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RGBA キャンバスプール
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CanvasPool {
    buf: Vec<u8>,
}

impl CanvasPool {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// 指定サイズの RGBA キャンバスを背景色で初期化して所有権を渡す。
    /// 直前に `return_buffer` で戻された `Vec` を再利用するため、
    /// 確保コストと初期化コストの両方が削減される。
    pub fn take(&mut self, w: u32, h: u32, bg: [u8; 4]) -> Vec<u8> {
        let needed = (w as usize) * (h as usize) * 4;
        let mut buf = std::mem::take(&mut self.buf);
        if buf.len() != needed {
            buf.resize(needed, 0);
        }
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
        buf
    }

    /// 使用済みバッファをプールに返却する。
    pub fn return_buffer(&mut self, buf: Vec<u8>) {
        // 既に何か持っているなら大きい方を残す（リサイズに備える）
        if buf.capacity() > self.buf.capacity() {
            self.buf = buf;
        }
    }

    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// フレームキャッシュ（直前 1 枚のみ）
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CachedFrame {
    pub fingerprint: FrameFingerprint,
    #[allow(dead_code)]
    pub png: Rc<Vec<u8>>,
    pub base64: Rc<String>,
}

#[derive(Default)]
pub struct FrameCache {
    last: Option<CachedFrame>,
}

impl FrameCache {
    pub fn new() -> Self {
        Self { last: None }
    }

    pub fn lookup(&self, fp: &FrameFingerprint) -> Option<&CachedFrame> {
        self.last.as_ref().filter(|c| &c.fingerprint == fp)
    }

    pub fn store(&mut self, frame: CachedFrame) {
        self.last = Some(frame);
    }

    pub fn clear(&mut self) {
        self.last = None;
    }

    #[allow(dead_code)]
    pub fn has_displayed(&self) -> bool {
        self.last.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_load_attempt_does_not_panic() {
        let _ = load_font_bytes();
    }

    #[test]
    fn glyph_cache_hits_on_second_lookup() {
        let Some(font) = font() else {
            // フォントが見つからない環境ではスキップ
            return;
        };
        let mut cache = GlyphCache::default();
        let g1 = cache.rasterize(font, 'A', 24.0);
        let g2 = cache.rasterize(font, 'A', 24.0);
        assert!(Rc::ptr_eq(&g1, &g2), "expected cache hit");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn glyph_cache_evicts_oldest() {
        let Some(font) = font() else {
            return;
        };
        let mut cache = GlyphCache::with_capacity(2);
        let _a = cache.rasterize(font, 'A', 16.0);
        let _b = cache.rasterize(font, 'B', 16.0);
        let _c = cache.rasterize(font, 'C', 16.0); // 'A' がエビクト
        assert_eq!(cache.len(), 2);

        // 'A' を再ラスタライズすると新規 Rc になる
        let a2 = cache.rasterize(font, 'A', 16.0);
        let a3 = cache.rasterize(font, 'A', 16.0);
        assert!(Rc::ptr_eq(&a2, &a3));
    }

    #[test]
    fn canvas_pool_reuses_buffer() {
        let mut pool = CanvasPool::new();
        let buf1 = pool.take(10, 10, [1, 2, 3, 4]);
        assert_eq!(buf1.len(), 400);
        assert_eq!(&buf1[0..4], &[1, 2, 3, 4]);
        let cap1 = buf1.capacity();
        pool.return_buffer(buf1);
        let buf2 = pool.take(10, 10, [5, 6, 7, 8]);
        // capacity が維持されている（同サイズ再要求では再確保しない）
        assert!(buf2.capacity() >= cap1);
        assert_eq!(&buf2[0..4], &[5, 6, 7, 8]);
    }

    #[test]
    fn canvas_pool_resizes_on_larger_request() {
        let mut pool = CanvasPool::new();
        let buf1 = pool.take(4, 4, [0; 4]);
        pool.return_buffer(buf1);
        let buf2 = pool.take(8, 8, [9, 9, 9, 9]);
        assert_eq!(buf2.len(), 8 * 8 * 4);
        assert_eq!(&buf2[0..4], &[9, 9, 9, 9]);
    }

    #[test]
    fn font_is_cached() {
        let f1 = font();
        let f2 = font();
        match (f1, f2) {
            (Some(a), Some(b)) => assert_eq!(a as *const _, b as *const _),
            (None, None) => {} // 環境にフォントなし
            _ => panic!("font() returned inconsistent results"),
        }
    }
}
