//! The text view's faces: each paragraph wrapped to the room, the lit one lit and the rest faint
//! as the body says, and the focus moving smoothly from line to line.

use super::Textures;
use slot_input::Millis;
use slot_music::TextBody;
use slot_ui::music::{music_row_px, music_text_width, Ink, TextFaces};
use slot_ui::Canvas;

use super::{MusicText, Spec};

pub(super) fn faces(
    text: &mut MusicText,
    compositor: &mut dyn Textures,
    canvas: Canvas,
    body: TextBody,
    now: Millis,
) -> TextFaces {
    // A menu row's size: read at arm's length, as the rest is.
    let px = music_row_px(true, 1) as u32;
    let width = music_text_width(false, canvas);
    let mut lines = Vec::with_capacity(body.lines.len());
    for (i, line) in body.lines.into_iter().enumerate() {
        let ink = match body.lit {
            Some(lit) if lit != Some(i) => Ink::Faint,
            _ => Ink::Lit,
        };
        let spec = Spec::Paragraph {
            text: line,
            ink,
            px,
        };
        lines.push(
            text.paragraphs
                .sync(compositor, text.theme, i, spec, width, None),
        );
    }
    TextFaces {
        lines,
        focus: text.motion.focus(body.focus as f32, now),
        fade: body.lit.is_some(),
        progress: body.progress,
    }
}
