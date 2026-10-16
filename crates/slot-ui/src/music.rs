use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::{arrows_hint_face, centred_hints, hint_face, UndoFace, HINT_H, LEGEND_GAP};
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD, MENU_PX};
use crate::quick_menu::{QUICK_EDGE, QUICK_PITCH};
use crate::slot_chrome::{edge, opening};
use crate::text;

// Type: one place to tune the music screens, and to make configurable later.
/// Menus (the music main screen, settings) use the quick menu's type and spacing for this many
/// rows or fewer; lists set more, closer together.
const MENU_ROWS: usize = 6;
/// List type as a share of its row spacing, from Normal: 24 px on 44 px rows. Small and Large
/// scale from it through their row counts.
const LIST_TYPE_PER_ROW: f32 = 24.0 / 44.0;
/// Action rows (Play All, Shuffle) and Now Playing's second lines, against their main type.
const ACTION_SCALE: f32 = 0.8;
const SECOND_PX: f32 = MENU_PX * ACTION_SCALE;
/// Titles, counts and Now Playing's time.
const SMALL_PX: f32 = 18.0;
const SMALL_H: u32 = 26;
const STATUS_PX: f32 = 26.0;
const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];

// Layout.
const TITLE_Y: f32 = 20.0;
const ROWS_Y: f32 = 60.0;
const BAR_INSET: f32 = 4.0;
const LEGEND_Y: f32 = 427.0;
const PROGRESS_Y: f32 = 50.0;
const PROGRESS_H: f32 = 3.0;
pub const ART_BOX: u32 = 240;
/// Now Playing: the picture and its text, centred between title and legend.
const NP_TOP: f32 = (PROGRESS_Y + LEGEND_Y - 8.0 - ART_BOX as f32) / 2.0;
const NP_LINE: f32 = 44.0;
/// Now Playing: the spacing of the smaller second lines.
const NP_SECOND_LINE: f32 = 36.0;
const NP_BAR_GAP: f32 = 8.0;
const STATUS_GAP: f32 = 18.0;
const ART_GAP: f32 = 24.0;
const CARET_GAP: f32 = 14.0;
/// The gap between a row's parts: icon, number, name, artist.
const PART_GAP: u32 = 14;
/// A long line rests at each end of its scroll, moving between in 2 px steps (each step is
/// an upload).
const MARQUEE_REST_MS: u64 = 1200;
const MARQUEE_PX_PER_S: u64 = 60;
const MARQUEE_STEP: u32 = 2;

pub type MusicFace = (TexId, u32, u32);

/// The music screen's textures, uploaded by the frontend when they change.
#[derive(Default)]
pub struct MusicFaces {
    pub title: Option<MusicFace>,
    pub page: Option<MusicFace>,
    pub rows: Vec<Option<MusicFace>>,
    /// Values at the right of their rows, with carets on the highlighted one.
    pub values: Vec<Option<MusicFace>>,
    pub carets: Option<[MusicFace; 2]>,
    pub selected: Option<usize>,
    /// Whether the screen has an album picture box, and the picture once loaded.
    pub art_box: bool,
    pub art: Option<MusicFace>,
    /// Progress from 0 to 1, drawn as a thin bar.
    pub progress: Option<f32>,
    pub legend: Vec<(TexId, u32)>,
    /// Now Playing: state symbols, queue place and time.
    pub status: Vec<MusicFace>,
    pub place: Option<MusicFace>,
    pub time: Option<MusicFace>,
    /// Lists: rows per page (which sets the spacing), and action rows at the top.
    pub per_page: usize,
    pub actions: usize,
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

pub fn music_status_face(glyph: char) -> UndoFace {
    glyph_face(glyph, STATUS_PX, MENU_H)
}

/// A symbol from the icon font, `px` high, centred in a face `h` tall.
fn glyph_face(glyph: char, px: f32, h: u32) -> UndoFace {
    let Some(font) = crate::icon::symbols_font() else {
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
            rgba[at..at + 4].copy_from_slice(&[MENU_INK[0], MENU_INK[1], MENU_INK[2], a]);
        }
    }
    UndoFace { rgba, w, h }
}

/// A line at full length; wider ones scroll through `music_window`.
pub fn music_line_face(text: &str, ink: LineInk) -> Option<UndoFace> {
    let (px, h) = match ink {
        LineInk::Small => (SMALL_PX, SMALL_H),
        LineInk::Second => (SECOND_PX, MENU_H),
        _ => (MENU_PX, MENU_H),
    };
    Some(padded(&[run(text, ink != LineInk::Lit, px, h)?]))
}

/// The type size for `per_page` rows: the quick menu's for menus, from the spacing for lists.
pub fn music_row_px(per_page: usize) -> f32 {
    match per_page {
        0..=MENU_ROWS => MENU_PX,
        n => (pitch(n) * LIST_TYPE_PER_ROW).round(),
    }
}

/// Action rows are a size down.
pub fn music_action_px(per_page: usize) -> f32 {
    (music_row_px(per_page) * ACTION_SCALE).round()
}

/// A list row: icon, dim number, name, dim artist; empty parts are skipped.
pub fn music_row_face(
    icon: Option<char>,
    before: &str,
    text: &str,
    after: &str,
    dim: bool,
    px: f32,
) -> Option<UndoFace> {
    let h = (px + 12.0).ceil() as u32;
    let parts: Vec<UndoFace> = [
        icon.map(|g| glyph_face(g, px * 0.9, h)),
        run(before, true, px, h),
        run(text, dim, px, h),
        run(after, true, px, h),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| padded(&parts))
}

/// Unpadded text, lit or dim, `px` high in a face `h` tall.
fn run(text: &str, dim: bool, px: f32, h: u32) -> Option<UndoFace> {
    let font = text::label_font()?;
    if text.is_empty() {
        return None;
    }
    let layout = text::fit(font, text, f32::INFINITY, 1, px, px);
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    // A pixel either side for glyphs that reach past their advance.
    let w = set.ceil() as u32 + 2;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let colour = if dim { DIM_INK } else { MENU_INK };
    text::draw_centred(&mut rgba, w, h, &layout, colour);
    Some(UndoFace { rgba, w, h })
}

/// `parts` side by side, with the menu's padding around them.
fn padded(parts: &[UndoFace]) -> UndoFace {
    let h = parts.iter().map(|p| p.h).max().unwrap_or(MENU_H);
    let inner: u32 =
        parts.iter().map(|p| p.w).sum::<u32>() + PART_GAP * (parts.len() as u32).saturating_sub(1);
    let w = inner + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut x = MENU_PAD;
    for p in parts {
        let top = (h - p.h) / 2;
        for y in 0..p.h {
            let src = (y * p.w * 4) as usize;
            let dst = (((top + y) * w + x) * 4) as usize;
            rgba[dst..dst + (p.w * 4) as usize]
                .copy_from_slice(&p.rgba[src..src + (p.w * 4) as usize]);
        }
        x += p.w + PART_GAP;
    }
    UndoFace { rgba, w, h }
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
    let t = ms % (2 * MARQUEE_REST_MS + travel);
    match t {
        t if t < MARQUEE_REST_MS => 0,
        t if t < MARQUEE_REST_MS + travel => {
            let px = ((t - MARQUEE_REST_MS) * MARQUEE_PX_PER_S / 1000) as u32;
            px - px % MARQUEE_STEP
        }
        _ => overflow,
    }
}

/// The visible width of a line, beside the picture or across the screen.
pub fn music_text_width(art: bool) -> u32 {
    OUT_W - QUICK_EDGE as u32 - text_left(art) + 2 * MENU_PAD
}

/// The title shares its line with the page count.
pub const MUSIC_TITLE_W: u32 = OUT_W / 2;

fn text_left(art: bool) -> u32 {
    match art {
        true => QUICK_EDGE as u32 + ART_BOX + ART_GAP as u32,
        false => QUICK_EDGE as u32,
    }
}

/// The button hints per kind of music screen.
pub struct MusicLegends<T> {
    pub main: Vec<T>,
    pub list: Vec<T>,
    pub queue: Vec<T>,
    pub now_playing: Vec<T>,
    pub settings: Vec<T>,
    pub back: Vec<T>,
}

impl<T> MusicLegends<T> {
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> MusicLegends<U> {
        MusicLegends {
            main: self.main.into_iter().map(&mut f).collect(),
            list: self.list.into_iter().map(&mut f).collect(),
            queue: self.queue.into_iter().map(&mut f).collect(),
            now_playing: self.now_playing.into_iter().map(&mut f).collect(),
            settings: self.settings.into_iter().map(&mut f).collect(),
            back: self.back.into_iter().map(&mut f).collect(),
        }
    }
}

pub fn music_legend_faces() -> MusicLegends<UndoFace> {
    MusicLegends {
        main: vec![
            hint_face("B", "Back"),
            hint_face("A", "Open"),
            hint_face("SELECT", "Playing"),
        ],
        list: vec![
            hint_face("B", "Back"),
            hint_face("A", "Open"),
            hint_face("X", "Add"),
            hint_face("Y", "Next"),
            hint_face("SELECT", "Playing"),
        ],
        queue: vec![
            hint_face("B", "Back"),
            hint_face("A", "Play"),
            hint_face("X", "Remove"),
            hint_face("L1 R1", "Move"),
        ],
        now_playing: vec![
            hint_face("B", "Back"),
            hint_face("A", "Pause"),
            hint_face("X", "Shuffle"),
            hint_face("Y", "Repeat"),
            hint_face("DOWN", "Queue"),
        ],
        settings: vec![hint_face("B", "Back"), arrows_hint_face("Change")],
        back: vec![hint_face("B", "Back")],
    }
}

/// Which lines scroll: the highlighted row, or all lines when nothing is highlighted.
pub fn music_scrolls(selected: Option<usize>, row: usize) -> bool {
    selected.is_none_or(|s| s == row)
}

pub fn draw_music(faces: &MusicFaces, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: opening(),
    });
    let pitch = pitch(faces.per_page);
    if let Some(at) = faces.selected {
        out.push(Draw::Rect {
            x: 0.0,
            y: row_top(at, pitch) + BAR_INSET,
            w: OUT_W as f32,
            h: pitch - 2.0 * BAR_INSET,
            colour: edge(),
        });
    }
    let pad = MENU_PAD as f32;
    if let Some((tex, w, h)) = faces.title {
        push(out, tex, QUICK_EDGE - pad, TITLE_Y, w, h);
    }
    if let Some((tex, w, h)) = faces.page {
        let x = OUT_W as f32 - QUICK_EDGE + pad - w as f32;
        push(out, tex, x, TITLE_Y, w, h);
    }
    match faces.art_box {
        true => draw_now_playing(faces, out),
        false => {
            if let Some(done) = faces.progress {
                draw_progress(
                    QUICK_EDGE,
                    PROGRESS_Y,
                    OUT_W as f32 - 2.0 * QUICK_EDGE,
                    done,
                    out,
                );
            }
            for (i, face) in faces.rows.iter().enumerate() {
                if let Some((tex, w, h)) = *face {
                    push(out, tex, QUICK_EDGE - pad, row_mid(i, pitch, h), w, h);
                }
            }
            draw_rule(faces, pitch, out);
            draw_values(faces, pitch, out);
        }
    }
    for (tex, w, x) in centred_hints(slot_gfx::Canvas::FRAMED, &faces.legend, LEGEND_GAP) {
        push(out, tex, x, LEGEND_Y, w, HINT_H);
    }
}

/// A thin rule under the action rows.
fn draw_rule(faces: &MusicFaces, pitch: f32, out: &mut Vec<Draw>) {
    if faces.actions == 0 || faces.actions >= faces.rows.len() {
        return;
    }
    let ink = DIM_INK.map(|c| c as f32 / 255.0);
    out.push(Draw::Rect {
        x: QUICK_EDGE,
        y: row_top(faces.actions, pitch).round(),
        w: OUT_W as f32 - 2.0 * QUICK_EDGE,
        h: 1.0,
        colour: [ink[0], ink[1], ink[2], 0.5],
    });
}

/// Values at the right edge, as in the quick menu, with carets on the highlighted one.
fn draw_values(faces: &MusicFaces, pitch: f32, out: &mut Vec<Draw>) {
    let (right, pad) = (OUT_W as f32 - QUICK_EDGE, MENU_PAD as f32);
    for (i, value) in faces.values.iter().enumerate() {
        let Some((tex, w, h)) = *value else {
            continue;
        };
        let y = row_mid(i, pitch, h);
        match (faces.carets, faces.selected == Some(i)) {
            (Some([(lt, lw, lh), (rt, rw, rh)]), true) => {
                let rx = right - rw as f32;
                push(out, rt, rx, row_mid(i, pitch, rh), rw, rh);
                let vx = rx - CARET_GAP + pad - w as f32;
                push(out, tex, vx, y, w, h);
                let lx = vx + pad - CARET_GAP - lw as f32;
                push(out, lt, lx, row_mid(i, pitch, lh), lw, lh);
            }
            _ => push(out, tex, right + pad - w as f32, y, w, h),
        }
    }
}

/// Now Playing: the picture with the text aligned to its edges. Names hang from its top; the
/// state symbols, bar, queue place and time sit on its bottom.
fn draw_now_playing(faces: &MusicFaces, out: &mut Vec<Draw>) {
    let pad = MENU_PAD as f32;
    match faces.art {
        Some((tex, w, h)) => {
            let x = QUICK_EDGE + (ART_BOX - w) as f32 / 2.0;
            let y = NP_TOP + (ART_BOX - h) as f32 / 2.0;
            push(out, tex, x, y, w, h);
        }
        None => out.push(Draw::Rect {
            x: QUICK_EDGE,
            y: NP_TOP,
            w: ART_BOX as f32,
            h: ART_BOX as f32,
            colour: edge(),
        }),
    }
    // The text keeps to the picture's edges, as players set it: the names hang from its
    // top, and the bar and time sit on its bottom.
    let top = NP_TOP;
    let (left, right) = (text_left(true) as f32, OUT_W as f32 - QUICK_EDGE);
    for (i, face) in faces.rows.iter().enumerate() {
        if let Some((tex, w, h)) = *face {
            let y = top + i.min(1) as f32 * NP_LINE + i.saturating_sub(1) as f32 * NP_SECOND_LINE;
            push(out, tex, left - pad, y, w, h);
        }
    }
    let bar = NP_TOP + ART_BOX as f32 - SMALL_H as f32 - NP_BAR_GAP - PROGRESS_H;
    // Play or pause at the left, shuffle and repeat at the right.
    let icons = |h: u32| bar - NP_BAR_GAP - h as f32;
    if let Some(&(tex, w, h)) = faces.status.first() {
        push(out, tex, left, icons(h), w, h);
    }
    let mut x = right;
    for &(tex, w, h) in faces.status.iter().skip(1).rev() {
        x -= w as f32;
        push(out, tex, x, icons(h), w, h);
        x -= STATUS_GAP;
    }
    draw_progress(left, bar, right - left, faces.progress.unwrap_or(0.0), out);
    let small = bar + PROGRESS_H + NP_BAR_GAP;
    if let Some((tex, w, h)) = faces.place {
        push(out, tex, left - pad, small, w, h);
    }
    if let Some((tex, w, h)) = faces.time {
        push(out, tex, right + pad - w as f32, small, w, h);
    }
}

/// A thin line, filled as far as `done` (0 to 1).
fn draw_progress(x: f32, y: f32, w: f32, done: f32, out: &mut Vec<Draw>) {
    let ink = MENU_INK.map(|c| c as f32 / 255.0);
    out.push(Draw::Rect {
        x,
        y,
        w,
        h: PROGRESS_H,
        colour: edge(),
    });
    out.push(Draw::Rect {
        x,
        y,
        w: (w * done.clamp(0.0, 1.0)).round(),
        h: PROGRESS_H,
        colour: [ink[0], ink[1], ink[2], 1.0],
    });
}

/// Row spacing: the quick menu's for 6 rows, closer for more.
fn pitch(per_page: usize) -> f32 {
    match per_page {
        0..=MENU_ROWS => QUICK_PITCH,
        n => ((LEGEND_Y - 8.0 - ROWS_Y) / n as f32).floor(),
    }
}

fn row_top(i: usize, pitch: f32) -> f32 {
    ROWS_Y + pitch * i as f32
}

/// The top of a face `h` tall, centred in row `i`.
fn row_mid(i: usize, pitch: f32, h: u32) -> f32 {
    row_top(i, pitch) + (pitch - h as f32) / 2.0
}

fn push(out: &mut Vec<Draw>, tex: TexId, x: f32, y: f32, w: u32, h: u32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_scale_from_normal_and_menus_keep_the_quick_menu_size() {
        assert_eq!(music_row_px(6), MENU_PX, "a menu");
        assert_eq!(music_row_px(8), 24.0, "Normal");
        assert!(music_row_px(10) < 24.0 && music_row_px(7) > 24.0);
        assert_eq!(pitch(6), QUICK_PITCH);
    }

    #[test]
    fn every_music_legend_fits_across_the_screen() {
        let l = music_legend_faces();
        for (name, hints) in [
            ("main", &l.main),
            ("list", &l.list),
            ("queue", &l.queue),
            ("now playing", &l.now_playing),
            ("settings", &l.settings),
            ("back", &l.back),
        ] {
            let w: u32 = hints.iter().map(|h| h.w).sum::<u32>()
                + LEGEND_GAP as u32 * (hints.len() as u32).saturating_sub(1);
            assert!(w <= OUT_W, "the {name} legend is {w} px wide");
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
}
