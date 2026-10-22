//! The music screens' textures: lines of text, list rows, symbols, and the window a long line
//! scrolls through. Each is drawn in a theme's colours.

use super::chrome::Layout;
use super::chrome::{
    arrows_hint_coloured, arrows_hint_width, blit, cap_width, draw_centred, fit, hint_coloured,
    hint_width, label_font, line_width, render_svg, symbols_font, Bitmap, CAP, MENU_H, MENU_PAD,
    MENU_PX,
};
use super::{
    MusicTheme, FIT_MIN, LETTER_BOX, LETTER_PX, MARQUEE_BEAT_MS, MARQUEE_REST_MS, MARQUEE_STEP,
    NO_ART_PX, PART_GAP, RING_PX, RING_STROKE, SECOND_PX, SMALL_H, SMALL_PX, STATUS_PX,
};

/// The music screens' inks, as slot's quick menu sets them, and one more; a theme says their
/// colours:
/// - `Lit`: what can be opened or played: names, menu rows, Play All.
/// - `Dim`: what describes it: numbers, artists, values, counts.
/// - `Faint`: what is behind or unavailable: played tracks, nothing to resume.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Ink {
    Lit,
    Dim,
    Faint,
}

/// How a line is set: lit, dim, a smaller dim second line, or small.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LineInk {
    Lit,
    Dim,
    /// Dim and a size down, like a track's artist.
    Second,
    Small,
}

/// State symbols from the icon font: play, pause, repeat, repeat one, shuffle.
pub const MUSIC_STATUS: [char; 5] = [
    '\u{f040a}',
    '\u{f03e4}',
    '\u{f0456}',
    '\u{f0458}',
    '\u{f049d}',
];
/// The carets either side of a setting's value, left then right.
const CARETS: [char; 2] = ['\u{f0d9}', '\u{f0da}'];
const CARET_PX: f32 = 24.0;

pub fn music_status_face(glyph: char, theme: &MusicTheme) -> Bitmap {
    glyph_face(glyph, STATUS_PX, MENU_H, theme.lit)
}

/// A caret beside a setting's value: left, or `right`.
pub fn music_caret_face(right: bool, theme: &MusicTheme) -> Bitmap {
    glyph_face(CARETS[usize::from(right)], CARET_PX, MENU_H, theme.lit)
}

/// Symbols the icon font slot carries does not have, drawn from the same icon set's shapes
/// (Material Design Icons) on a 24 unit square.
const HEART: &str = "M12,21.35L10.55,20.03C5.4,15.36 2,12.27 2,8.5C2,5.41 4.42,3 7.5,3C9.24,3 \
    10.91,3.81 12,4.88C13.09,3.81 14.76,3 16.5,3C19.58,3 22,5.41 22,8.5C22,12.27 18.6,15.36 \
    13.45,20.03L12,21.35Z";
/// What the square shows with no picture: a note for a song, a book for a book.
const NOTE: &str =
    "M12 3V13.55C11.41 13.21 10.73 13 10 13C7.79 13 6 14.79 6 17S7.79 21 10 21 14 19.21 14 \
    17V7H18V3H12Z";
const BOOK: &str =
    "M18,22A2,2 0 0,0 20,20V4C20,2.89 19.1,2 18,2H12V9L9.5,7.5L7,9V2H6A2,2 0 0,0 4,4V20A2,2 \
    0 0,0 6,22H18Z";

/// A favorite's heart, beside the state symbols.
pub fn music_heart_face(theme: &MusicTheme) -> Bitmap {
    icon(HEART, STATUS_PX, MENU_H, theme.lit)
}

/// What Now Playing's square shows with no picture, as players have it: a note, or a `book`.
pub fn music_no_art_face(book: bool, theme: &MusicTheme) -> Bitmap {
    let path = if book { BOOK } else { NOTE };
    icon(path, NO_ART_PX, NO_ART_PX.round() as u32, theme.dim)
}

/// What a list's empty thumbnail shows, as Now Playing's square does, at its size.
pub fn music_thumb_blank_face(book: bool, theme: &MusicTheme) -> Bitmap {
    let path = if book { BOOK } else { NOTE };
    let px = super::THUMB_PX as f32 * 0.5;
    icon(path, px, px.round() as u32, theme.dim)
}

/// A favorite's heart at the right of a list row, sized to the row's type.
pub fn music_row_heart_face(menu: bool, per_page: usize, theme: &MusicTheme) -> Bitmap {
    let px = super::music_row_px(menu, per_page);
    icon(HEART, px * 0.8, (px + 12.0).ceil() as u32, theme.lit)
}

/// The shape `path` `px` across, in `ink`, centred in a face `h` tall.
fn icon(path: &str, px: f32, h: u32, ink: [u8; 3]) -> Bitmap {
    let [r, g, b] = ink;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{path}" fill="#{r:02x}{g:02x}{b:02x}"/></svg>"##
    );
    let side = (px.round() as u32).min(h);
    let mut rgba = vec![0u8; (side * h * 4) as usize];
    if let Some(icon) = render_svg(&svg, side, side) {
        blit(&mut rgba, side, &icon, side, side, 0, (h - side) / 2);
    }
    Bitmap { rgba, w: side, h }
}

/// A line at full length, set a little smaller to fit `fit_w` if that is enough; wider ones
/// scroll through `music_window`.
pub fn music_line_face(text: &str, ink: LineInk, fit_w: u32, theme: &MusicTheme) -> Option<Bitmap> {
    let (px, h) = match ink {
        LineInk::Small => (SMALL_PX, SMALL_H),
        LineInk::Second => (SECOND_PX, MENU_H),
        LineInk::Lit | LineInk::Dim => (MENU_PX, MENU_H),
    };
    let tone = match ink {
        LineInk::Lit => theme.lit,
        _ => theme.dim,
    };
    // A second line is set in its own case, a level under the name above it.
    let cased = ink == LineInk::Second;
    fitted(fit_w, px, |px| {
        Some(padded(&[set(text, tone, px, h, cased)?]))
    })
}

/// A two-line row's second line: who made it, dim, `px` high and in its own case, a level under
/// the name above; as tall as a row's face is for its size, so the two stack alike.
pub fn music_second_face(text: &str, px: f32, fit_w: u32, theme: &MusicTheme) -> Option<Bitmap> {
    let h = (px + 2.0 * ROW_FACE_PAD).ceil() as u32;
    fitted(fit_w, px, |px| {
        Some(padded(&[set(text, theme.dim, px, h, true)?]))
    })
}

/// A paragraph in `ink`, wrapped to `width` and each line centred, as the text view shows its
/// lines; none for an empty one, a gap.
pub fn music_paragraph_face(
    text: &str,
    ink: Ink,
    px: f32,
    width: u32,
    theme: &MusicTheme,
) -> Option<Bitmap> {
    let font = label_font()?;
    if text.trim().is_empty() {
        return None;
    }
    let layout = fit(font, text, width as f32, PARAGRAPH_LINES, px, px);
    let line_h = font.horizontal_line_metrics(px)?.new_line_size;
    let h = (line_h * layout.lines.len() as f32).ceil() as u32;
    // As wide as its widest line, and a pixel either side for glyphs past their advance.
    let widest = layout
        .lines
        .iter()
        .map(|l| line_width(font, l, layout.px, layout.tracking));
    let w = (widest.fold(0.0, f32::max).ceil() as u32 + 2).min(width);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    draw_centred(&mut rgba, w, h, &layout, theme.ink(ink));
    Some(Bitmap { rgba, w, h })
}

/// The most lines one paragraph wraps to.
const PARAGRAPH_LINES: usize = 8;

/// A list row's words, in order: a mark, a number, the name, and what follows it, a name or
/// what a key does. Empty parts are skipped. The name is a label in capitals, or, `cased`, a
/// sentence set as written; what follows it is set as written, as a second line is.
#[derive(Copy, Clone, Default)]
pub struct RowWords<'a> {
    pub icon: Option<char>,
    pub before: &'a str,
    pub text: &'a str,
    pub after: &'a str,
    pub cased: bool,
}

impl<'a> RowWords<'a> {
    /// A row of a name alone, as a label.
    pub fn name(text: &'a str) -> RowWords<'a> {
        RowWords {
            text,
            ..RowWords::default()
        }
    }
}

/// A list row: its name in `ink`, the number and what follows dim, or faint with a faint name.
/// Fitted to `fit_w` as lines are.
pub fn music_row_face(
    words: RowWords,
    ink: Ink,
    px: f32,
    fit_w: u32,
    theme: &MusicTheme,
) -> Option<Bitmap> {
    let RowWords {
        icon,
        before,
        text,
        after,
        cased,
    } = words;
    let h = (px + 2.0 * ROW_FACE_PAD).ceil() as u32;
    let context = match ink {
        Ink::Faint => Ink::Faint,
        _ => Ink::Dim,
    };
    let (name, context) = (theme.ink(ink), theme.ink(context));
    fitted(fit_w, px, |px| {
        // A mark stands in a number's place, in its room, so the name keeps in line.
        let number = run(before, context, px, h);
        let mark = icon.map(|g| glyph_face(g, px * 0.9, h, name));
        let (mark, number) = match (mark, number) {
            (Some(mark), Some(room)) => (Some(centred_in(mark, room.w)), None),
            other => other,
        };
        let parts: Vec<Bitmap> = [
            mark,
            number,
            set(text, name, px, h, cased),
            set(after, context, px, h, true),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| padded(&parts))
    })
}

/// `build` at `px`, or a few sizes down when that fits it in `w`: a line that nearly fits
/// scrolling a few pixels looks broken.
fn fitted(w: u32, px: f32, build: impl Fn(f32) -> Option<Bitmap>) -> Option<Bitmap> {
    let face = build(px)?;
    if face.w <= w {
        return Some(face);
    }
    let mut at = (px * w as f32 / face.w as f32).floor();
    while at >= (px * FIT_MIN).ceil() {
        match build(at) {
            Some(small) if small.w <= w => return Some(small),
            _ => at -= 1.0,
        }
    }
    Some(face)
}

/// The letter a list jumped to, large, for the box in the middle of the screen.
pub fn music_letter_face(letter: char, theme: &MusicTheme) -> Option<Bitmap> {
    run(
        &letter.to_string(),
        theme.ground,
        LETTER_PX,
        LETTER_BOX as u32,
    )
}

/// The ring that fills as a key is held, round from the top: its track `faint`, the part
/// held `done` (0 to 1) in `ink`. Let go before it closes, and nothing happens.
pub fn music_ring_face(done: f32, ink: [u8; 3], theme: &MusicTheme) -> Bitmap {
    let side = RING_PX;
    let centre = side as f32 / 2.0;
    let outer = centre - 1.0;
    let inner = outer - RING_STROKE;
    let mut rgba = vec![0; (side * side * 4) as usize];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = ((i as u32 % side) as f32, (i as u32 / side) as f32);
        let (dx, dy) = (x + 0.5 - centre, y + 0.5 - centre);
        let r = dx.hypot(dy);
        // Antialiased at both edges.
        let cover = (outer - r + 0.5).min(r - inner + 0.5).clamp(0.0, 1.0);
        let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
        px[..3].copy_from_slice(if turn <= done { &ink } else { &theme.faint });
        px[3] = (cover * 255.0).round() as u8;
    }
    Bitmap {
        rgba,
        w: side,
        h: side,
    }
}

/// How far a hint's ring sits outside its key cap, and how round its corners are.
pub(super) const CAP_RING_GAP: u32 = 4;
const CAP_RING_RADIUS: f32 = 8.0;

/// The ring that fills around a hint's key cap as the key is held, round from the top: a
/// rounded outline just outside the cap (`key`'s, or the arrows' for `None`), its track `faint`,
/// the part held `done` (0 to 1) in `ink`.
pub fn music_cap_ring_face(
    key: Option<&str>,
    done: f32,
    ink: [u8; 3],
    theme: &MusicTheme,
) -> Bitmap {
    let caps = match key {
        Some(key) => cap_width(key),
        None => arrows_hint_width("") - (hint_width("A", "") - cap_width("A")),
    };
    let (w, h) = (caps + 2 * CAP_RING_GAP, CAP + 2 * CAP_RING_GAP);
    let (hw, hh) = (w as f32 / 2.0, h as f32 / 2.0);
    let r = CAP_RING_RADIUS.min(hh);
    let mut rgba = vec![0; (w * h * 4) as usize];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (
            (i as u32 % w) as f32 + 0.5 - hw,
            (i as u32 / w) as f32 + 0.5 - hh,
        );
        // How far outside the rounded outline's edge, negative within.
        let (qx, qy) = (x.abs() - (hw - 0.5) + r, y.abs() - (hh - 0.5) + r);
        let edge = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
        let cover = (0.5 - edge).min(edge + RING_STROKE + 0.5).clamp(0.0, 1.0);
        let turn = ((x / hw).atan2(-y / hh) / std::f32::consts::TAU).rem_euclid(1.0);
        px[..3].copy_from_slice(if turn <= done { &ink } else { &theme.faint });
        px[3] = (cover * 255.0).round() as u8;
    }
    Bitmap { rgba, w, h }
}

/// A button hint in the theme: slot's, with its key caps filled in `lit` and lettered in the
/// ground, so they read on a light screen too. `None` is the Left and Right arrows.
pub fn music_hint_face(key: Option<&str>, label: &str, theme: &MusicTheme) -> Bitmap {
    let (mut face, caps) = match key {
        Some(key) => (hint_coloured(key, label, theme.lit), cap_width(key)),
        None => {
            // The arrows' caps take what a key's cap leaves of a hint.
            let label_w = hint_width("A", label) - cap_width("A");
            let face = arrows_hint_coloured(label, theme.lit);
            (face, arrows_hint_width(label) - label_w)
        }
    };
    recolour_caps(&mut face, caps, theme.lit, theme.ground);
    face
}

/// Slot draws a cap as a light fill with dark lettering, blended between; this keeps each
/// pixel's blend and puts `fill` and `letter` in their places. Only opaque pixels in the first
/// `caps` columns are caps.
fn recolour_caps(face: &mut Bitmap, caps: u32, fill: [u8; 3], letter: [u8; 3]) {
    let w = face.w as usize;
    let cap_pixels = |rgba: &[u8]| -> Vec<usize> {
        (0..rgba.len() / 4)
            .filter(|&p| p % w < caps as usize && rgba[p * 4 + 3] == 255)
            .collect()
    };
    let pixels = cap_pixels(&face.rgba);
    let level = |p: usize| face.rgba[p * 4] as f32;
    let (dark, light) = pixels.iter().fold((255.0f32, 0.0f32), |(lo, hi), &p| {
        (lo.min(level(p)), hi.max(level(p)))
    });
    if light <= dark {
        return;
    }
    for p in pixels {
        let t = (face.rgba[p * 4] as f32 - dark) / (light - dark);
        for c in 0..3 {
            let v = letter[c] as f32 + (fill[c] as f32 - letter[c] as f32) * t;
            face.rgba[p * 4 + c] = v.round() as u8;
        }
    }
}

/// How long after `ms` the line `overflow` px too long next moves, as `marquee_offset` moves it:
/// none for a line that fits. Drawn only then, a scrolling line costs a frame a step, not one a
/// frame, and none while it rests; and every change falls on the beat.
pub fn marquee_next(overflow: u32, ms: u64) -> Option<u64> {
    if overflow == 0 {
        return None;
    }
    let travel = marquee_travel(overflow);
    let period = 2 * MARQUEE_REST_MS + travel;
    let (t, end) = (ms % period, MARQUEE_REST_MS + travel);
    let next = if t < end {
        MARQUEE_REST_MS
            + (t.saturating_sub(MARQUEE_REST_MS) / MARQUEE_BEAT_MS + 1) * MARQUEE_BEAT_MS
    } else {
        period
    };
    Some(next - t)
}

/// When a line starting to scroll `now` starts: on the beat, so it steps with the rest.
pub fn marquee_start(now: u64) -> u64 {
    now - now % MARQUEE_BEAT_MS
}

/// How long a line `overflow` px too long takes to scroll its length: whole beats.
fn marquee_travel(overflow: u32) -> u64 {
    u64::from(overflow.div_ceil(MARQUEE_STEP)) * MARQUEE_BEAT_MS
}

/// Columns `x` to `x + w` of `face`: the visible part of a long line.
pub fn music_window(face: &Bitmap, x: u32, w: u32) -> Bitmap {
    let x = x.min(face.w.saturating_sub(w));
    let w = w.min(face.w);
    let mut rgba = Vec::with_capacity((w * face.h * 4) as usize);
    for row in face.rgba.chunks_exact((face.w * 4) as usize) {
        rgba.extend_from_slice(&row[(x * 4) as usize..((x + w) * 4) as usize]);
    }
    // An edge with more of the line beyond it fades out, so a cut line reads as going on; its
    // padding is left clear there, so the ink stops at the margin as a whole line's does.
    let (left, right) = (x > 0, x + w < face.w);
    for row in rgba.chunks_exact_mut((w * 4) as usize) {
        for i in 0..w.min(MENU_PAD + WINDOW_FADE) {
            let keep = (i.saturating_sub(MENU_PAD) as f32 + 0.5) / WINDOW_FADE as f32;
            let keep = if i < MENU_PAD { 0.0 } else { keep };
            let mut fade = |at: u32| {
                let a = &mut row[(at * 4 + 3) as usize];
                *a = (f32::from(*a) * keep).round() as u8;
            };
            if right {
                fade(w - 1 - i);
            }
            if left {
                fade(i);
            }
        }
    }
    Bitmap { rgba, w, h: face.h }
}

/// How far a cut line's edge fades out, in pixels.
const WINDOW_FADE: u32 = 28;

/// How far a line `overflow` px too long has scrolled after `ms`: rest, scroll, rest, repeat.
pub fn marquee_offset(overflow: u32, ms: u64) -> u32 {
    if overflow == 0 {
        return 0;
    }
    let travel = marquee_travel(overflow);
    match ms % (2 * MARQUEE_REST_MS + travel) {
        t if t < MARQUEE_REST_MS => 0,
        t if t < MARQUEE_REST_MS + travel => {
            let steps = ((t - MARQUEE_REST_MS) / MARQUEE_BEAT_MS) as u32;
            (steps * MARQUEE_STEP).min(overflow)
        }
        _ => overflow,
    }
}

/// A symbol from the icon font in `ink`, `px` high, centred in a face `h` tall.
fn glyph_face(glyph: char, px: f32, h: u32, ink: [u8; 3]) -> Bitmap {
    let Some(font) = symbols_font() else {
        return Bitmap {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let (m, cov) = font.rasterize(glyph, px);
    let w = m.width as u32;
    let top = ((h as i32 - m.height as i32) / 2).max(0) as u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for gy in 0..(m.height as u32).min(h - top) {
        for gx in 0..w {
            let at = (((top + gy) * w + gx) * 4) as usize;
            let a = cov[(gy * w + gx) as usize];
            rgba[at..at + 4].copy_from_slice(&[ink[0], ink[1], ink[2], a]);
        }
    }
    Bitmap { rgba, w, h }
}

/// Unpadded text in `ink`, `px` high in a face `h` tall, in capitals as slot sets its labels.
fn run(text: &str, ink: [u8; 3], px: f32, h: u32) -> Option<Bitmap> {
    set(text, ink, px, h, false)
}

/// As `run`, or, `cased`, in the case it is written in: a second line under a name, set apart
/// from it as players set an artist under a title.
fn set(text: &str, ink: [u8; 3], px: f32, h: u32, cased: bool) -> Option<Bitmap> {
    let font = label_font()?;
    if text.is_empty() {
        return None;
    }
    let layout = fit(font, text, f32::INFINITY, 1, px, px);
    let layout = if cased {
        Layout {
            lines: vec![text.to_string()],
            ..layout
        }
    } else {
        layout
    };
    let w = set_w(font, &layout);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    draw_centred(&mut rgba, w, h, &layout, ink);
    Some(Bitmap { rgba, w, h })
}

/// How wide `layout` is set, a pixel either side for glyphs that reach past their advance.
fn set_w(font: &fontdue::Font, layout: &Layout) -> u32 {
    let set = layout
        .lines
        .iter()
        .map(|l| line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    set.ceil() as u32 + 2
}

/// How far a row's name starts past its number at `px`: the number and the gap after it, as
/// `music_row_face` sets them; 0 with none. A second line under the name starts there too.
pub fn music_number_w(number: &str, px: f32) -> u32 {
    match label_font() {
        Some(font) if !number.is_empty() => {
            set_w(font, &fit(font, number, f32::INFINITY, 1, px, px)) + PART_GAP
        }
        _ => 0,
    }
}

/// `parts` side by side, centred on one line, with the menu's padding around them.
/// The blank above and below a row's type in its face, so rows of any size centre alike.
pub(super) const ROW_FACE_PAD: f32 = 6.0;

/// `face` centred across `w`, as wide as what it stands in for, if wider than it.
fn centred_in(face: Bitmap, w: u32) -> Bitmap {
    let w = w.max(face.w);
    let mut rgba = vec![0u8; (w * face.h * 4) as usize];
    blit(
        &mut rgba,
        w,
        &face.rgba,
        face.w,
        face.h,
        (w - face.w) / 2,
        0,
    );
    Bitmap { rgba, w, h: face.h }
}

fn padded(parts: &[Bitmap]) -> Bitmap {
    let h = parts.iter().map(|p| p.h).max().unwrap_or(MENU_H);
    let gaps = PART_GAP * (parts.len() as u32).saturating_sub(1);
    let w = parts.iter().map(|p| p.w).sum::<u32>() + gaps + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut x = MENU_PAD;
    for p in parts {
        blit(&mut rgba, w, &p.rgba, p.w, p.h, x, (h - p.h) / 2);
        x += p.w + PART_GAP;
    }
    Bitmap { rgba, w, h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_line_fades_out_at_an_edge_with_more_beyond_it_and_only_there() {
        let face = Bitmap {
            rgba: vec![255; 200 * 2 * 4],
            w: 200,
            h: 2,
        };
        let alpha = |f: &Bitmap, x: u32| f.rgba[(x * 4 + 3) as usize];
        let start = music_window(&face, 0, 120);
        assert_eq!(alpha(&start, 0), 255, "its start, nothing before it");
        assert!(alpha(&start, 119) < 16, "faded where it goes on");
        assert_eq!(alpha(&start, 60), 255);
        let pad = 120 - MENU_PAD;
        assert_eq!(alpha(&start, pad), 0, "clear in the padding, at the margin");
        assert!(alpha(&start, pad - 1) > 0, "fading in from there");
        let middle = music_window(&face, 40, 120);
        assert!(
            alpha(&middle, 0) < 16 && alpha(&middle, 119) < 16,
            "both ways"
        );
        let end = music_window(&face, 80, 120);
        assert!(
            alpha(&end, 0) < 16 && alpha(&end, 119) == 255,
            "its end, nothing after it"
        );
    }

    #[test]
    fn a_cap_s_ring_sits_just_outside_it_and_fills_from_the_top() {
        let theme = MusicTheme::dark();
        let ink = [0xff, 0, 0];
        let ring = music_cap_ring_face(Some("START"), 0.1, ink, &theme);
        let gap = 2 * CAP_RING_GAP;
        assert_eq!((ring.w, ring.h), (cap_width("START") + gap, CAP + gap));
        let px = |x: u32, y: u32| &ring.rgba[((y * ring.w + x) * 4) as usize..][..4];
        assert_eq!(px(ring.w / 2 + 1, 1)[..3], ink, "begun at the top");
        assert_eq!(px(ring.w / 2 - 2, 1)[..3], theme.faint, "and not yet round");
        assert_eq!(
            px(ring.w / 2, ring.h / 2)[3],
            0,
            "the cap itself left clear"
        );
    }

    #[test]
    fn a_mark_stands_in_its_number_s_room_so_the_name_keeps_in_line() {
        let theme = MusicTheme::dark();
        let row = |icon| RowWords {
            icon,
            before: "01",
            ..RowWords::name("A")
        };
        let face = |icon| music_row_face(row(icon), Ink::Lit, 24.0, u32::MAX, &theme).unwrap();
        let mark = MUSIC_STATUS[0];
        assert_eq!(
            face(Some(mark)).w,
            face(None).w,
            "as wide, the name where it was"
        );
    }

    #[test]
    fn a_row_s_number_is_measured_as_its_face_sets_it_and_none_is_nothing() {
        let theme = MusicTheme::dark();
        let row = |before| RowWords {
            before,
            ..RowWords::name("A")
        };
        let with = music_row_face(row("01"), Ink::Lit, 24.0, u32::MAX, &theme);
        let without = music_row_face(row(""), Ink::Lit, 24.0, u32::MAX, &theme);
        let (with, without) = (with.unwrap().w, without.unwrap().w);
        assert_eq!(
            music_number_w("01", 24.0),
            with - without,
            "where the name moves to"
        );
        assert_eq!(music_number_w("", 24.0), 0);
    }

    #[test]
    fn the_heart_is_drawn_as_tall_as_the_state_symbols() {
        let face = music_heart_face(&MusicTheme::dark());
        assert_eq!(face.h, MENU_H);
        assert!(face.rgba.chunks_exact(4).any(|p| p[3] == 255), "drawn");
    }

    #[test]
    fn a_song_with_no_picture_shows_a_note_and_a_book_a_book() {
        let theme = MusicTheme::dark();
        let [note, book] = [false, true].map(|book| music_no_art_face(book, &theme));
        for face in [&note, &book] {
            assert_eq!((face.w, face.h), (96, 96), "two fifths of the square");
            assert!(face.rgba.chunks_exact(4).any(|p| p[3] == 255), "drawn");
        }
        assert_ne!(note.rgba, book.rgba);
    }

    #[test]
    fn the_icon_font_has_every_symbol() {
        let font = symbols_font().unwrap();
        for glyph in MUSIC_STATUS {
            assert_ne!(font.lookup_glyph_index(glyph), 0, "{glyph:?}");
        }
    }

    #[test]
    fn a_scrolling_line_says_when_it_next_moves_and_never_moves_before() {
        assert_eq!(marquee_next(0, 5_000), None, "a line that fits never moves");
        for overflow in [1, 2, 3, 7, 120, 121] {
            let travel = marquee_travel(overflow);
            let period = 2 * MARQUEE_REST_MS + travel;
            let mut wakes = 0;
            let mut ms = 0;
            while ms < 2 * period {
                let wait = marquee_next(overflow, ms).unwrap();
                let at = marquee_offset(overflow, ms);
                for d in 1..wait {
                    assert_eq!(
                        marquee_offset(overflow, ms + d),
                        at,
                        "{overflow} px at {ms} + {d}"
                    );
                }
                (ms, wakes) = (ms + wait, wakes + 1);
            }
            // A wake a step, and one at each end, not one a frame.
            let steps = u64::from(overflow / MARQUEE_STEP) + 3;
            assert!(wakes <= 2 * steps, "{overflow} px: {wakes} wakes");
        }
    }

    #[test]
    fn every_long_line_moves_only_on_the_one_beat_so_they_step_together() {
        // Two lines, of other lengths, begun at other moments: each changes only on the beat.
        for (overflow, begun) in [(120, 1_000), (37, 4_321), (1, 77)] {
            let start = marquee_start(begun);
            for now in start + 1..start + 20_000 {
                let (was, is) = (now - 1 - start, now - start);
                if marquee_offset(overflow, was) != marquee_offset(overflow, is) {
                    assert_eq!(now % MARQUEE_BEAT_MS, 0, "{overflow} px moved at {now}");
                }
            }
        }
    }

    #[test]
    fn a_long_line_rests_scrolls_rests_and_starts_over() {
        assert_eq!(marquee_offset(0, 5_000), 0);
        assert_eq!(marquee_offset(120, 0), 0);
        assert_eq!(
            marquee_offset(120, MARQUEE_REST_MS + 1000),
            50,
            "50 px a second"
        );
        let travel = marquee_travel(120);
        assert_eq!(marquee_offset(120, MARQUEE_REST_MS + travel + 10), 120);
        assert_eq!(marquee_offset(120, 2 * MARQUEE_REST_MS + travel), 0);
    }

    #[test]
    fn a_line_that_nearly_fits_is_set_smaller_and_a_long_one_scrolls() {
        let full = music_line_face(
            "Benjamin Francis Leftwich",
            LineInk::Lit,
            u32::MAX,
            &MusicTheme::dark(),
        )
        .unwrap();
        let near = music_line_face(
            "Benjamin Francis Leftwich",
            LineInk::Lit,
            full.w - 8,
            &MusicTheme::dark(),
        )
        .unwrap();
        assert!(near.w <= full.w - 8 && near.h == full.h);
        let far = music_line_face(
            "Benjamin Francis Leftwich",
            LineInk::Lit,
            full.w / 2,
            &MusicTheme::dark(),
        )
        .unwrap();
        assert_eq!(far.w, full.w);
    }

    #[test]
    fn a_window_is_the_columns_it_names_and_never_reads_past_the_line() {
        let face = Bitmap {
            rgba: (0..2 * 5 * 4).map(|i| i as u8).collect(),
            w: 5,
            h: 2,
        };
        // Its colours, column for column; its alpha fades where the line goes on, as tested apart.
        let colours = |px: &[u8]| -> Vec<u8> {
            px.iter()
                .enumerate()
                .filter(|(i, _)| i % 4 != 3)
                .map(|(_, &c)| c)
                .collect()
        };
        let w = music_window(&face, 1, 2);
        assert_eq!((w.w, w.h), (2, 2));
        assert_eq!(colours(&w.rgba[..8]), colours(&face.rgba[4..12]));
        assert_eq!(colours(&w.rgba[8..]), colours(&face.rgba[24..32]));
        let w = music_window(&face, 9, 3);
        assert_eq!(colours(&w.rgba[..12]), colours(&face.rgba[8..20]));
        assert_eq!(music_window(&face, 0, 9).w, 5);
    }

    #[test]
    fn parts_sit_side_by_side_inside_the_padding() {
        let part = |w| Bitmap {
            rgba: vec![255; (w * 2 * 4) as usize],
            w,
            h: 2,
        };
        let face = padded(&[part(3), part(4)]);
        assert_eq!(face.w, 3 + PART_GAP + 4 + 2 * MENU_PAD);
        let lit = |x: u32| face.rgba[(x * 4 + 3) as usize] == 255;
        assert!(!lit(MENU_PAD - 1) && lit(MENU_PAD) && lit(MENU_PAD + 2));
        assert!(!lit(MENU_PAD + 3) && lit(MENU_PAD + 3 + PART_GAP));
    }
}
