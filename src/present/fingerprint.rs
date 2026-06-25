//! フレーム同一性判定用の fingerprint。
//!
//! 「現在の状態で描画されるべき画像」を一意に表す。
//! `compute(app, canvas)` で生成し、前回と等しければキャッシュヒット。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::app::{App, ExecStatus};
use crate::model::BlockKind;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameFingerprint {
    pub slide_idx: usize,
    pub slide_hash: u64,
    pub font_size: u8,
    pub canvas_size: (u32, u32),
    pub selected_block: Option<usize>,
    pub running_output_hash: u64,
    pub running_status: u8,
    pub status_message_hash: u64,
}

impl FrameFingerprint {
    pub fn compute(app: &App, canvas_size: (u32, u32)) -> Self {
        let slide = app.current_slide();
        let mut h = DefaultHasher::new();
        for b in &slide.blocks {
            b.id.hash(&mut h);
            std::mem::discriminant(&b.kind).hash(&mut h);
            b.content.hash(&mut h);
            match &b.kind {
                BlockKind::Exec { lang, signature } => {
                    lang.hash(&mut h);
                    signature.hash(&mut h);
                }
                BlockKind::Code { lang } => lang.hash(&mut h),
                BlockKind::Heading { level } => level.hash(&mut h),
                _ => {}
            }
        }
        let slide_hash = h.finish();

        let (running_output_hash, running_status) = match app.running_exec.as_ref() {
            Some(r) if r.slide_idx == app.current_slide => {
                let mut h2 = DefaultHasher::new();
                r.buffer.hash(&mut h2);
                r.scroll.hash(&mut h2);
                (h2.finish(), exec_status_code(r.status))
            }
            _ => (0, 0),
        };

        let mut sh = DefaultHasher::new();
        match &app.status_message {
            Some(s) => {
                1u8.hash(&mut sh);
                s.hash(&mut sh);
            }
            None => 0u8.hash(&mut sh),
        }
        let status_message_hash = sh.finish();

        Self {
            slide_idx: app.current_slide,
            slide_hash,
            font_size: app.presentation.font_size,
            canvas_size,
            selected_block: app.selected_block,
            running_output_hash,
            running_status,
            status_message_hash,
        }
    }
}

fn exec_status_code(s: ExecStatus) -> u8 {
    match s {
        ExecStatus::Running => 1,
        ExecStatus::Completed(_) => 2,
        ExecStatus::Failed(_) => 3,
        ExecStatus::Cancelled => 4,
        ExecStatus::SpawnError => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::Presentation;

    fn fp(app: &App) -> FrameFingerprint {
        FrameFingerprint::compute(app, (960, 576))
    }

    #[test]
    fn fingerprint_stable_for_same_state() {
        let app = App::new(Presentation::demo());
        assert_eq!(fp(&app), fp(&app));
    }

    #[test]
    fn fingerprint_changes_on_slide_change() {
        let mut app = App::new(Presentation::demo());
        let a = fp(&app);
        app.next_slide();
        let b = fp(&app);
        assert_ne!(a, b, "slide change must invalidate fingerprint");
    }

    #[test]
    fn fingerprint_changes_on_font_size_change() {
        let mut app = App::new(Presentation::demo());
        let a = fp(&app);
        app.set_font_size(app.presentation.font_size + 1);
        let b = fp(&app);
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_changes_on_canvas_resize() {
        let app = App::new(Presentation::demo());
        let a = FrameFingerprint::compute(&app, (960, 576));
        let b = FrameFingerprint::compute(&app, (1280, 720));
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_changes_on_selected_block_change() {
        let mut app = App::new(Presentation::demo());
        let a = fp(&app);
        app.next_block();
        let b = fp(&app);
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_changes_on_status_message_change() {
        let mut app = App::new(Presentation::demo());
        let a = fp(&app);
        app.set_status("changed".to_string());
        let b = fp(&app);
        assert_ne!(a, b);
    }
}
