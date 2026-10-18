//! The music screens' textures: lines of text, list rows, symbols, and the window a long line
//! scrolls through. Each is drawn in a theme's colours.

use super::slot::{
    arrows_hint_coloured, arrows_hint_width, blit, cap_width, draw_centred, fit, hint_coloured,
    hint_width, label_font, line_width, render_svg, symbols_font, UndoFace, MENU_H, MENU_PAD,
    MENU_PX,
};
use super::{
    MusicTheme, FIT_MIN, LETTER_BOX, LETTER_PX, MARQUEE_PX_PER_S, MARQUEE_REST_MS, MARQUEE_STEP,
    PART_GAP, RING_PX, RING_STROKE, SECOND_PX, SMALL_H, SMALL_PX, STATUS_PX,
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

pub fn music_status_face(glyph: char, theme: &MusicTheme) -> UndoFace {
    glyph_face(glyph, STATUS_PX, MENU_H, theme.lit)
}

/// A caret beside a setting's value: left, or `right`.
pub fn music_caret_face(right: bool, theme: &MusicTheme) -> UndoFace {
    glyph_face(CARETS[usize::from(right)], CARET_PX, MENU_H, theme.lit)
}

/// Symbols the icon font slot carries does not have, drawn from the same icon set's shapes
/// (Material Design Icons) on a 24 unit square.
const HEART: &str = "M12,21.35L10.55,20.03C5.4,15.36 2,12.27 2,8.5C2,5.41 4.42,3 7.5,3C9.24,3 \
    10.91,3.81 12,4.88C13.09,3.81 14.76,3 16.5,3C19.58,3 22,5.41 22,8.5C22,12.27 18.6,15.36 \
    13.45,20.03L12,21.35Z";
const MOON: &str = "M2 12A10 10 0 0 0 15 21.54A10 10 0 0 1 15 2.46A10 10 0 0 0 2 12Z";

/// A favorite's heart, beside the state symbols.
pub fn music_heart_face(theme: &MusicTheme) -> UndoFace {
    icon(HEART, STATUS_PX, MENU_H, theme.lit)
}

/// The sleep timer's moon, beside the state symbols while it counts down.
pub fn music_moon_face(theme: &MusicTheme) -> UndoFace {
    icon(MOON, STATUS_PX * 0.85, MENU_H, theme.lit)
}

/// A favorite's heart at the right of a list row, sized to the row's type.
pub fn music_row_heart_face(menu: bool, per_page: usize, theme: &MusicTheme) -> UndoFace {
    let px = super::music_row_px(menu, per_page);
    icon(HEART, px * 0.8, (px + 12.0).ceil() as u32, theme.lit)
}

/// The shape `path` `px` across, in `ink`, centred in a face `h` tall.
fn icon(path: &str, px: f32, h: u32, ink: [u8; 3]) -> UndoFace {
    let [r, g, b] = ink;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{path}" fill="#{r:02x}{g:02x}{b:02x}"/></svg>"##
    );
    let side = (px.round() as u32).min(h);
    let mut rgba = vec![0u8; (side * h * 4) as usize];
    if let Some(icon) = render_svg(&svg, side, side) {
        blit(&mut rgba, side, &icon, side, side, 0, (h - side) / 2);
    }
    UndoFace { rgba, w: side, h }
}

/// A line at full length, set a little smaller to fit `fit_w` if that is enough; wider ones
/// scroll through `music_window`.
pub fn music_line_face(
    text: &str,
    ink: LineInk,
    fit_w: u32,
    theme: &MusicTheme,
) -> Option<UndoFace> {
    let (px, h) = match ink {
        LineInk::Small => (SMALL_PX, SMALL_H),
        LineInk::Second => (SECOND_PX, MENU_H),
        LineInk::Lit | LineInk::Dim => (MENU_PX, MENU_H),
    };
    let tone = match ink {
        LineInk::Lit => theme.lit,
        _ => theme.dim,
    };
    fitted(fit_w, px, |px| Some(padded(&[run(text, tone, px, h)?])))
}

/// A paragraph in `ink`, wrapped to `width` and each line centred, as the text view shows its
/// lines; none for an empty one, a gap.
pub fn music_paragraph_face(
    text: &str,
    ink: Ink,
    px: f32,
    width: u32,
    theme: &MusicTheme,
) -> Option<UndoFace> {
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
    Some(UndoFace { rgba, w, h })
}

/// The most lines one paragraph wraps to.
const PARAGRAPH_LINES: usize = 8;

/// A list row: icon, number, name in `ink`, artist; empty parts are skipped. The number and
/// artist are dim, or faint with a faint name. Fitted to `fit_w` as lines are.
#[allow(clippy::too_many_arguments)]
pub fn music_row_face(
    icon: Option<char>,
    before: &str,
    text: &str,
    after: &str,
    ink: Ink,
    px: f32,
    fit_w: u32,
    theme: &MusicTheme,
) -> Option<UndoFace> {
    let h = (px + 12.0).ceil() as u32;
    let context = match ink {
        Ink::Faint => Ink::Faint,
        _ => Ink::Dim,
    };
    let (name, context) = (theme.ink(ink), theme.ink(context));
    fitted(fit_w, px, |px| {
        let parts: Vec<UndoFace> = [
            icon.map(|g| glyph_face(g, px * 0.9, h, name)),
            run(before, context, px, h),
            run(text, name, px, h),
            run(after, context, px, h),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| padded(&parts))
    })
}

/// `build` at `px`, or a few sizes down when that fits it in `w`: a line that nearly fits
/// scrolling a few pixels looks broken.
fn fitted(w: u32, px: f32, build: impl Fn(f32) -> Option<UndoFace>) -> Option<UndoFace> {
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
pub fn music_letter_face(letter: char, theme: &MusicTheme) -> Option<UndoFace> {
    run(&letter.to_string(), theme.lit, LETTER_PX, LETTER_BOX as u32)
}

/// The ring that fills as a key is held, round from the top: its track `faint`, the part
/// held `done` (0 to 1) in `ink`. Let go before it closes, and nothing happens.
pub fn music_ring_face(done: f32, ink: [u8; 3], theme: &MusicTheme) -> UndoFace {
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
    UndoFace {
        rgba,
        w: side,
        h: side,
    }
}

/// A button hint in the theme: slot's, with its key caps filled in `lit` and lettered in the
/// ground, so they read on a light screen too. `None` is the Left and Right arrows.
pub fn music_hint_face(key: Option<&str>, label: &str, theme: &MusicTheme) -> UndoFace {
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
fn recolour_caps(face: &mut UndoFace, caps: u32, fill: [u8; 3], letter: [u8; 3]) {
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

/// Columns `x` to `x + w` of `face`: the visible part of a long line.
pub fn music_window(face: &UndoFace, x: u32, w: u32) -> UndoFace {
    let x = x.min(face.w.saturating_sub(w));
    let w = w.min(face.w);
    let mut rgba = Vec::with_capacity((w * face.h * 4) as usize);
    for row in face.rgba.chunks_exact((face.w * 4) as usize) {
        rgba.extend_from_slice(&row[(x * 4) as usize..((x + w) * 4) as usize]);
    }
    UndoFace { rgba, w, h: face.h }
}

/// How far a line `overflow` px too long has scrolled after `ms`: rest, scroll, rest, repeat.
pub fn marquee_offset(overflow: u32, ms: u64) -> u32 {
    if overflow == 0 {
        return 0;
    }
    let travel = overflow as u64 * 1000 / MARQUEE_PX_PER_S;
    match ms % (2 * MARQUEE_REST_MS + travel) {
        t if t < MARQUEE_REST_MS => 0,
        t if t < MARQUEE_REST_MS + travel => {
            let px = ((t - MARQUEE_REST_MS) * MARQUEE_PX_PER_S / 1000) as u32;
            px - px % MARQUEE_STEP
        }
        _ => overflow,
    }
}

/// A symbol from the icon font in `ink`, `px` high, centred in a face `h` tall.
fn glyph_face(glyph: char, px: f32, h: u32, ink: [u8; 3]) -> UndoFace {
    let Some(font) = symbols_font() else {
        return UndoFace {
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
    UndoFace { rgba, w, h }
}

/// Unpadded text in `ink`, `px` high in a face `h` tall.
fn run(text: &str, ink: [u8; 3], px: f32, h: u32) -> Option<UndoFace> {
    let font = label_font()?;
    if text.is_empty() {
        return None;
    }
    let layout = fit(font, text, f32::INFINITY, 1, px, px);
    let set = layout
        .lines
        .iter()
        .map(|l| line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    // A pixel either side for glyphs that reach past their advance.
    let w = set.ceil() as u32 + 2;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    draw_centred(&mut rgba, w, h, &layout, ink);
    Some(UndoFace { rgba, w, h })
}

/// `parts` side by side, centred on one line, with the menu's padding around them.
fn padded(parts: &[UndoFace]) -> UndoFace {
    let h = parts.iter().map(|p| p.h).max().unwrap_or(MENU_H);
    let gaps = PART_GAP * (parts.len() as u32).saturating_sub(1);
    let w = parts.iter().map(|p| p.w).sum::<u32>() + gaps + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut x = MENU_PAD;
    for p in parts {
        blit(&mut rgba, w, &p.rgba, p.w, p.h, x, (h - p.h) / 2);
        x += p.w + PART_GAP;
    }
    UndoFace { rgba, w, h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_heart_and_moon_are_drawn_as_tall_as_the_state_symbols() {
        let theme = MusicTheme::dark();
        for face in [music_heart_face(&theme), music_moon_face(&theme)] {
            assert_eq!(face.h, MENU_H);
            assert!(face.rgba.chunks_exact(4).any(|p| p[3] == 255), "drawn");
        }
    }

    #[test]
    fn the_icon_font_has_every_symbol() {
        let font = symbols_font().unwrap();
        for glyph in MUSIC_STATUS {
            assert_ne!(font.lookup_glyph_index(glyph), 0, "{glyph:?}");
        }
    }

    #[test]
    fn a_long_line_rests_scrolls_rests_and_starts_over() {
        assert_eq!(marquee_offset(0, 5_000), 0);
        assert_eq!(marquee_offset(120, 0), 0);
        assert_eq!(marquee_offset(120, MARQUEE_REST_MS + 1000), 60);
        let travel = 120 * 1000 / MARQUEE_PX_PER_S;
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
        let face = UndoFace {
            rgba: (0..2 * 5 * 4).map(|i| i as u8).collect(),
            w: 5,
            h: 2,
        };
        let w = music_window(&face, 1, 2);
        assert_eq!((w.w, w.h), (2, 2));
        assert_eq!(&w.rgba[..8], &face.rgba[4..12]);
        assert_eq!(&w.rgba[8..], &face.rgba[24..32]);
        let w = music_window(&face, 9, 3);
        assert_eq!(&w.rgba[..12], &face.rgba[8..20]);
        assert_eq!(music_window(&face, 0, 9).w, 5);
    }

    #[test]
    fn parts_sit_side_by_side_inside_the_padding() {
        let part = |w| UndoFace {
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
