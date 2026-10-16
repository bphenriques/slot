//! Now Playing: the picture with the text kept to its edges, as players set it. The names hang
//! from its top; the state symbols, bar, time and queue place sit on its bottom. The next track
//! is centred below, above the hints.

use super::slot::{push, Canvas, Draw, MENU_PAD, QUICK_EDGE};

/// A bookmark's tick on the bar: its width, and how far it stands above and below.
const MARK_W: f32 = 2.0;
const MARK_RISE: f32 = 3.0;
use super::theme::rgba;
use super::{
    draw_progress, text_left, MusicFace, MusicTheme, MusicTint, NowPlayingFaces, ART_BOX, LEGEND_Y,
    NP_BAR_GAP, NP_LINE, NP_SECOND_LINE, NP_TOP, PROGRESS_H, SMALL_H, STATUS_GAP,
};

/// `art_in` is how far the picture has faded in, 0 to 1.
pub(super) fn draw(
    np: &NowPlayingFaces,
    art_in: f32,
    canvas: Canvas,
    theme: &MusicTheme,
    out: &mut Vec<Draw>,
) {
    draw_art(np, art_in, theme, out);
    let pad = MENU_PAD as f32;
    let (left, right) = (text_left(true) as f32, canvas.w - QUICK_EDGE);
    for (i, face) in np.lines.iter().enumerate() {
        if let Some((tex, w, h)) = *face {
            let y =
                NP_TOP + i.min(1) as f32 * NP_LINE + i.saturating_sub(1) as f32 * NP_SECOND_LINE;
            push(out, tex, left - pad, y, w, h);
        }
    }
    let bar = NP_TOP + ART_BOX as f32 - SMALL_H as f32 - NP_BAR_GAP - PROGRESS_H;
    // Play or pause at the left, shuffle and repeat at the right.
    let icons = |h: u32| bar - NP_BAR_GAP - h as f32;
    if let Some(&(tex, w, h)) = np.status.first() {
        push(out, tex, left, icons(h), w, h);
    }
    let mut x = right;
    for &(tex, w, h) in np.status.iter().skip(1).rev() {
        x -= w as f32;
        push(out, tex, x, icons(h), w, h);
        x -= STATUS_GAP;
    }
    let ink = MusicTint::bar_ink(np.tint.as_ref(), art_in, theme);
    draw_progress(
        left,
        bar,
        right - left,
        np.progress.unwrap_or(0.0),
        ink,
        theme,
        out,
    );
    // Bookmarks stand up from the bar, as ticks.
    for &at in &np.marks {
        out.push(Draw::Rect {
            x: (left + (right - left) * at - MARK_W / 2.0).round(),
            y: bar - MARK_RISE,
            w: MARK_W,
            h: PROGRESS_H + 2.0 * MARK_RISE,
            colour: rgba(theme.lit, 1.0),
        });
    }
    let small = bar + PROGRESS_H + NP_BAR_GAP;
    // The time under the bar's start, where it grows from; the queue place at the end.
    if let Some((tex, w, h)) = np.time {
        push(out, tex, left - pad, small, w, h);
    }
    if let Some((tex, w, h)) = np.place {
        push(out, tex, right + pad - w as f32, small, w, h);
    }
    // Up Next, midway between the picture and the hints.
    if let Some((tex, w, h)) = np.next {
        let y = ((NP_TOP + ART_BOX as f32 + LEGEND_Y - h as f32) / 2.0).round();
        push(out, tex, canvas.centred(w as f32).round(), y, w, h);
    }
}

/// The picture centred in its box, fading in over the last one. The empty box shows only
/// before the first picture: between two, it would flash grey round a picture that is not
/// square, so a last picture of another shape fades out instead.
fn draw_art(np: &NowPlayingFaces, art_in: f32, theme: &MusicTheme, out: &mut Vec<Draw>) {
    let shown = np.art.map_or(0.0, |_| art_in);
    let was = np.art.and(np.art_was).filter(|_| shown < 1.0);
    if was.is_none() {
        let [r, g, b, a] = rgba(theme.bar, 1.0);
        out.push(Draw::Rect {
            x: QUICK_EDGE,
            y: NP_TOP,
            w: ART_BOX as f32,
            h: ART_BOX as f32,
            colour: [r, g, b, a * (1.0 - shown)],
        });
    }
    let same = |(_, w, h): MusicFace| np.art.is_some_and(|(_, nw, nh)| (nw, nh) == (w, h));
    let was_alpha = was.map_or(0.0, |f| if same(f) { 1.0 } else { 1.0 - shown });
    for (face, alpha) in [(was, was_alpha), (np.art, shown)] {
        if let Some((tex, w, h)) = face {
            let x = QUICK_EDGE + (ART_BOX - w) as f32 / 2.0;
            let y = NP_TOP + (ART_BOX - h) as f32 / 2.0;
            push(out, tex, x, y, w, h);
            if let Some(Draw::Tex { alpha: a, .. }) = out.last_mut() {
                *a = alpha;
            }
        }
    }
}
