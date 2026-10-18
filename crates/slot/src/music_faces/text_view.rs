//! The text view's faces: each paragraph wrapped to the room, the lit one lit and the rest dim
//! as the body says, and the focus moving smoothly from line to line.

use slot_gfx::Compositor;
use slot_input::Millis;
use slot_music::TextBody;
use slot_ui::{music_row_px, music_text_width, Canvas, Ink, TextFaces};

use super::{MusicText, Spec};

pub(super) fn faces(
    text: &mut MusicText,
    compositor: &mut Compositor,
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
            Some(lit) if lit != Some(i) => Ink::Dim,
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
    }
}
