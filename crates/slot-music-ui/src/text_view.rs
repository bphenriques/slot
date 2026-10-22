//! A text view: paragraphs one under another, centred, the one in focus held in the middle of
//! the room between the header and the hints, or the song's bar above them; as lyrics are shown.

#[cfg(test)]
use super::chrome::TexId;
use super::chrome::{push, Canvas, Draw, MENU_PAD, QUICK_EDGE};
use super::theme::rgba;
use super::{draw_progress, MusicFace, MusicTheme, LEGEND_Y, PROGRESS_H, ROWS_Y, SMALL_H};

/// Room between paragraphs, and what an empty one, a pause, takes.
const GAP: f32 = 12.0;
const PAUSE_H: f32 = 24.0;
/// Lines followed as they are sung fade further the further they are from the one in the middle,
/// down to the faintest, so the eye stays on it: how much a line, and how far.
const FADE_STEP: f32 = 0.3;
const FADE_MOST: f32 = 0.7;
/// The song's time sits this far above the hints, and its bar this far above the time, as on
/// Now Playing.
const FOOT_GAP: f32 = 12.0;
const TIME_GAP: f32 = 4.0;

/// The text view's paragraphs, and which is in the middle: between two as it scrolls; whether
/// those away from it fade, as lyrics followed do; and how far into the song, as Now Playing
/// shows it under its picture: a bar above the hints, in the album's ink if it has a picture,
/// and the time under its start.
#[derive(Default)]
pub struct TextFaces {
    pub lines: Vec<Option<MusicFace>>,
    pub focus: f32,
    pub fade: bool,
    pub progress: Option<f32>,
    pub tint: Option<[u8; 3]>,
    pub time: Option<MusicFace>,
}

pub(super) fn draw(text: &TextFaces, canvas: Canvas, theme: &MusicTheme, out: &mut Vec<Draw>) {
    // The song's time just above the hints, its bar above it, and the lyrics in the room over.
    let time_y = LEGEND_Y - FOOT_GAP - SMALL_H as f32;
    let bar_y = time_y - TIME_GAP - PROGRESS_H;
    let bottom = if text.progress.is_some() {
        bar_y
    } else {
        LEGEND_Y
    };
    if let Some(done) = text.progress {
        let (w, ink) = (
            canvas.w - 2.0 * QUICK_EDGE,
            rgba(text.tint.unwrap_or(theme.lit), 1.0),
        );
        draw_progress(QUICK_EDGE, bar_y, w, done, ink, theme, out);
    }
    if let Some((tex, w, h)) = text.time {
        push(out, tex, QUICK_EDGE - MENU_PAD as f32, time_y, w, h);
    }
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
    let shift = ((ROWS_Y + bottom) / 2.0 - at.min(last + PAUSE_H)).round();
    for (i, (line, top)) in text.lines.iter().zip(&tops).enumerate() {
        let Some((tex, w, h)) = *line else {
            continue;
        };
        let y = top + shift;
        // Only what is wholly between the header and the hints.
        if y >= ROWS_Y && y + h as f32 <= bottom - GAP {
            push(out, tex, ((canvas.w - w as f32) / 2.0).round(), y, w, h);
            if let (true, Some(Draw::Tex { alpha, .. })) = (text.fade, out.last_mut()) {
                *alpha = faded((i as f32 - text.focus).abs());
            }
        }
    }
}

/// How strongly a line `away` lines from the middle shows: the one there and those next to it
/// whole, as their ink has them, then fainter a line at a time.
fn faded(away: f32) -> f32 {
    1.0 - ((away - 1.0).max(0.0) * FADE_STEP).min(FADE_MOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_focus_sits_in_the_middle_and_what_will_not_fit_is_left_out() {
        let faces = TextFaces {
            lines: (0..40).map(|i| Some((TexId(i), 200, 30))).collect(),
            focus: 20.0,
            ..TextFaces::default()
        };
        let mut out = Vec::new();
        draw(&faces, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
        assert!(!out.is_empty() && out.len() < 40, "only those in the room");
        let mid = (ROWS_Y + LEGEND_Y) / 2.0;
        let focus = out.iter().find_map(|d| match d {
            Draw::Tex { tex, y, .. } if *tex == TexId(20) => Some(*y),
            _ => None,
        });
        let y = focus.expect("the focus is drawn");
        assert!((y + 15.0 - mid).abs() < 1.0, "centred: {y}");
    }

    #[test]
    fn followed_lines_fade_the_further_from_the_middle_and_the_song_s_bar_is_drawn() {
        let alpha_of = |fade: bool| {
            let faces = TextFaces {
                lines: (0..9).map(|i| Some((TexId(i), 200, 30))).collect(),
                focus: 4.0,
                fade,
                progress: Some(0.5),
                tint: Some([10, 200, 30]),
                time: Some((TexId(99), 120, 26)),
            };
            let mut out = Vec::new();
            draw(&faces, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
            let alpha = |i| {
                out.iter().find_map(|d| match *d {
                    Draw::Tex { tex, alpha, .. } if tex == TexId(i) => Some(alpha),
                    _ => None,
                })
            };
            // The bar in the album's ink, the time under it, and every line above the bar.
            let ink = rgba([10, 200, 30], 1.0);
            let bar_y = out.iter().find_map(|d| match *d {
                Draw::Rect { y, colour, .. } if colour == ink => Some(y),
                _ => None,
            });
            let tops = |d: &Draw, id: usize| match *d {
                Draw::Tex { tex, y, h, .. } if tex == TexId(id) => Some((y, h)),
                _ => None,
            };
            let time = out.iter().find_map(|d| tops(d, 99));
            let lowest = (0..9)
                .filter_map(|i| out.iter().find_map(|d| tops(d, i)))
                .map(|(y, h)| y + h)
                .fold(0.0, f32::max);
            let bar = bar_y
                .zip(time)
                .is_some_and(|(b, (t, _))| t > b && lowest < b && t + 26.0 <= LEGEND_Y);

            ([4, 3, 2, 1].map(alpha), bar)
        };
        let ([middle, next, two, three], bar) = alpha_of(true);
        assert_eq!(
            (middle, next),
            (Some(1.0), Some(1.0)),
            "the line and those beside it"
        );
        assert!(
            two < next && three < two,
            "fainter a line at a time: {two:?} {three:?}"
        );
        assert!(
            bar,
            "the song's bar above the hints in the album's ink, its time below, the lyrics above"
        );
        let (plain, _) = alpha_of(false);
        assert!(
            plain.iter().all(|&a| a == Some(1.0)),
            "plain lyrics, all to be read"
        );
    }
}
