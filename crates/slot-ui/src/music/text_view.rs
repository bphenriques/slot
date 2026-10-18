//! A text view: paragraphs one under another, centred, the one in focus held in the middle of
//! the room between the header and the hints; as lyrics are shown.

#[cfg(test)]
use super::slot::TexId;
use super::slot::{push, Canvas, Draw};
use super::{MusicFace, LEGEND_Y, ROWS_Y};

/// Room between paragraphs, and what an empty one, a pause, takes.
const GAP: f32 = 12.0;
const PAUSE_H: f32 = 24.0;

/// The text view's paragraphs, and which is in the middle: between two as it scrolls.
#[derive(Default)]
pub struct TextFaces {
    pub lines: Vec<Option<MusicFace>>,
    pub focus: f32,
}

pub(super) fn draw(text: &TextFaces, canvas: Canvas, out: &mut Vec<Draw>) {
    let height = |line: &Option<MusicFace>| line.map_or(PAUSE_H, |(_, _, h)| h as f32);
    // Each paragraph's top, the first's at 0.
    let mut tops = Vec::with_capacity(text.lines.len());
    let mut y = 0.0;
    for line in &text.lines {
        tops.push(y);
        y += height(line) + GAP;
    }
    let Some(&last) = tops.last() else {
        return;
    };
    // The focus's middle, part way to the next while it moves, set in the room's middle.
    let i = (text.focus.floor().max(0.0) as usize).min(tops.len() - 1);
    let middle = |i: usize| tops[i] + height(&text.lines[i]) / 2.0;
    let next = middle((i + 1).min(tops.len() - 1));
    let at = middle(i) + (next - middle(i)) * text.focus.fract();
    let shift = ((ROWS_Y + LEGEND_Y) / 2.0 - at.min(last + PAUSE_H)).round();
    for (line, top) in text.lines.iter().zip(&tops) {
        let Some((tex, w, h)) = *line else {
            continue;
        };
        let y = top + shift;
        // Only what is wholly between the header and the hints.
        if y >= ROWS_Y && y + h as f32 <= LEGEND_Y - GAP {
            push(out, tex, ((canvas.w - w as f32) / 2.0).round(), y, w, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_focus_sits_in_the_middle_and_what_will_not_fit_is_left_out() {
        let faces = TextFaces {
            lines: (0..40)
                .map(|i| Some((TexId::from_raw(i), 200, 30)))
                .collect(),
            focus: 20.0,
        };
        let mut out = Vec::new();
        draw(&faces, Canvas::FRAMED, &mut out);
        assert!(!out.is_empty() && out.len() < 40, "only those in the room");
        let mid = (ROWS_Y + LEGEND_Y) / 2.0;
        let focus = out.iter().find_map(|d| match d {
            Draw::Tex { tex, y, .. } if *tex == TexId::from_raw(20) => Some(*y),
            _ => None,
        });
        let y = focus.expect("the focus is drawn");
        assert!((y + 15.0 - mid).abs() < 1.0, "centred: {y}");
    }
}
