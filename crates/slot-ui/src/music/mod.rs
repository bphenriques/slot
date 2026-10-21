//! The music screens' kit: it draws a `slot_music::Screen` and knows nothing else of the
//! player. Every screen is made of the same few parts, each meaning one thing wherever it shows:
//!
//! - **Menu**: rows at slot's quick-menu size, centred, no header.
//! - **List**: rows sized by List Size under a title, with a count in the corner.
//! - **Row**: a name lit, what describes it dim, faint when behind or unavailable; ▶ marks what
//!   plays. An action row (Play All) is a size down, above a rule.
//! - **Corner**: short news: a count, a notice, the sleep timer.
//! - **Hints**: what each button does here and now; "Play (Next)" is a tap and a hold. A key held
//!   to act fills a ring around its cap.
//! - **Text view**: paragraphs, centred, the one in focus held in the middle.
//! - **Motion**: everything that moves eases as `tween` says; colours are all `theme`'s.
//!
//! The frontend (`slot::music_faces`) turns a `Screen` into these faces; another kit could take
//! this one's place behind that contract.

#![warn(clippy::match_bool, clippy::undocumented_unsafe_blocks)]

mod face;
mod list;
mod now_playing;
mod slot;
mod text_view;
mod theme;
mod tint;
mod tween;

use slot::{
    centred_hints, push, Canvas, Draw, TexId, CAP, HINT_H, LEGEND_GAP, MENU_PAD, MENU_PX, OUT_H,
    PLATE_H, QUICK_EDGE,
};

pub use face::{
    marquee_next, marquee_offset, marquee_start, music_cap_ring_face, music_caret_face,
    music_heart_face, music_hint_face, music_letter_face, music_line_face, music_no_art_face,
    music_paragraph_face, music_ring_face, music_row_face, music_row_heart_face, music_second_face,
    music_status_face, music_thumb_blank_face, music_window, Ink, LineInk, MUSIC_STATUS,
};
pub use list::{music_rich_px, music_row_ink, music_row_px, music_row_text_w};
pub use text_view::TextFaces;
pub use theme::MusicTheme;
pub use tint::{music_tint, music_tint_ink, MusicTint};
pub use tween::{music_eased, MusicTween, MUSIC_ART_MS, MUSIC_GLIDE_MS, MUSIC_SLIDE_MS};

// Type.
/// List type as a share of its row spacing, from Normal: 24 px on 44 px rows. Small and Large
/// scale from it through their row counts.
const LIST_TYPE_PER_ROW: f32 = 24.0 / 44.0;
/// Now Playing's second lines, against its main type.
const SECOND_PX: f32 = MENU_PX * 0.8;
/// Titles, counts and Now Playing's time.
const SMALL_PX: f32 = 18.0;
const SMALL_H: u32 = 26;
const STATUS_PX: f32 = 26.0;

// Layout.
/// The header sits in the middle of the band slot's volume and status use at the top, as slot's
/// own top row does, so the volume shows over it whole rather than across it.
const TITLE_Y: f32 = (PLATE_H - SMALL_H as f32) / 2.0;
const ROWS_Y: f32 = 60.0;
const BAR_INSET: f32 = 4.0;
/// The room a row's heart takes from its text.
const MUSIC_HEART_ROOM: u32 = 40;
const LEGEND_Y: f32 = 427.0;
const PROGRESS_Y: f32 = 50.0;
const PROGRESS_H: f32 = 3.0;
pub const ART_BOX: u32 = 240;
/// A list row's album thumbnail, square, inside the highlight of a two-line row (five a page),
/// and the room it takes before the row's text.
pub const THUMB_PX: u32 = 44;
const THUMB_ROOM: f32 = THUMB_PX as f32 + 12.0;
/// The note, or a book, on the square with no picture: two fifths of it across.
const NO_ART_PX: f32 = ART_BOX as f32 * 0.4;
/// Now Playing: the room kept under the picture for Up Next, so the screen does not move when
/// it comes and goes.
const UP_NEXT_ROOM: f32 = 40.0;
/// Now Playing: the picture and its text, centred between the title and Up Next.
const NP_TOP: f32 = (PROGRESS_Y + LEGEND_Y - 8.0 - UP_NEXT_ROOM - ART_BOX as f32) / 2.0;
const NP_LINE: f32 = 44.0;
/// Now Playing: the spacing of the smaller second lines.
const NP_SECOND_LINE: f32 = 36.0;
const NP_BAR_GAP: f32 = 8.0;
const STATUS_GAP: f32 = 18.0;
const ART_GAP: f32 = 24.0;
const CARET_GAP: f32 = 14.0;
/// The gap between a row's parts: icon, number, name, artist.
const PART_GAP: u32 = 14;
/// Between the title and what the corner says.
const CORNER_GAP: u32 = 24;
/// A long line rests at each end of its scroll, moving between in 2 px steps, one a beat (each
/// step is an upload and a frame): 50 px a second. Every long line keeps the one beat, so all of
/// them step on the same frames.
const MARQUEE_REST_MS: u64 = 1200;
const MARQUEE_STEP: u32 = 2;
const MARQUEE_BEAT_MS: u64 = 40;
/// A line a little too long is set smaller to fit, down to this share of its size, before it
/// scrolls.
const FIT_MIN: f32 = 0.88;

// Motion: its durations and curve are in `tween`.
const SLIDE_PX: f32 = 56.0;
/// The box the letter a list jumped to shows in, and the letter's size.
const LETTER_BOX: f32 = 96.0;
const LETTER_PX: f32 = 64.0;
/// The ring a held key fills in the corner, and how thick it is.
const RING_PX: u32 = 22;
const RING_STROKE: f32 = 3.0;

/// How many steps a held key's ring fills in: each is drawn once, so the frontend keeps them.
pub const MUSIC_RING_STEPS: f32 = 40.0;

/// A texture and its size.
pub type MusicFace = (TexId, u32, u32);

/// The music screen's textures, uploaded by the frontend when they change.
pub struct MusicFaces {
    pub title: Option<MusicFace>,
    pub corner: Option<MusicFace>,
    pub legend: Vec<(TexId, u32)>,
    pub body: BodyFaces,
    pub motion: MusicMotion,
    /// The room slot gives the screen, and the colours to draw in.
    pub canvas: Canvas,
    pub theme: MusicTheme,
    /// The letter a list just jumped to, large in the middle.
    pub letter: Option<MusicFace>,
    /// The ring a key held to act fills: around its hint's key cap, the hint the legend's
    /// `Some`; else in the corner, in place of its news.
    pub ring: Option<(MusicFace, Option<usize>)>,
}

impl Default for MusicFaces {
    fn default() -> Self {
        MusicFaces {
            title: None,
            corner: None,
            legend: Vec::new(),
            body: BodyFaces::default(),
            motion: MusicMotion::default(),
            canvas: Canvas::FRAMED,
            theme: MusicTheme::default(),
            letter: None,
            ring: None,
        }
    }
}

pub enum BodyFaces {
    List(ListFaces),
    NowPlaying(NowPlayingFaces),
    Text(TextFaces),
}

impl Default for BodyFaces {
    fn default() -> Self {
        BodyFaces::List(ListFaces::default())
    }
}

/// A page of rows, or a lone message line when nothing is highlighted.
#[derive(Default)]
pub struct ListFaces {
    pub rows: Vec<Option<MusicFace>>,
    /// Values at the right of their rows, with carets on the highlighted one.
    pub values: Vec<Option<MusicFace>>,
    pub carets: Option<[MusicFace; 2]>,
    pub selected: Option<usize>,
    /// Where the highlight is drawn, in rows: between two while it glides.
    pub bar: Option<f32>,
    /// Menus keep the quick menu's spacing, centred on `per_page` rows; lists fit them.
    pub menu: bool,
    pub per_page: usize,
    /// How many action rows, like Play All, lead the page: they sit closer together.
    pub head: usize,
    /// How far each row is from its place, in rows, as it slides there after the list changed.
    pub offsets: Vec<f32>,
    /// Rows with a rule above them.
    pub rules: Vec<usize>,
    /// Progress from 0 to 1, drawn as a thin bar under the title.
    pub progress: Option<f32>,
    /// The highlighted row is lifted, carried to a new place: its bar is lighter.
    pub lifted: bool,
    /// The rows that are favorites, and the heart drawn at their right.
    pub hearts: Vec<bool>,
    pub heart: Option<MusicFace>,
    /// Whether a title sits above it: a titled menu starts below it rather than centred.
    pub titled: bool,
    /// Whether the rows are two lines beside their album's thumbnail; each one's thumbnail,
    /// once read, and its second line: who made it.
    pub thumbed: bool,
    pub thumbs: Vec<Option<MusicFace>>,
    pub seconds: Vec<Option<MusicFace>>,
    /// What an empty thumbnail shows, dim: a note, or a book.
    pub blank: Option<MusicFace>,
    /// The row playing, in a list with thumbnails, and the play mark drawn on its thumbnail
    /// there rather than before its name, as players mark what plays on its artwork.
    pub playing: Option<usize>,
    pub play: Option<MusicFace>,
}

/// The track playing: its three lines, picture, state symbols, bar, place and time.
#[derive(Default)]
pub struct NowPlayingFaces {
    pub lines: [Option<MusicFace>; 3],
    pub art: Option<MusicFace>,
    /// The last picture, under the new one while it fades in.
    pub art_was: Option<MusicFace>,
    /// What the square shows until a picture comes, or with none: a note, or a book.
    pub no_art: Option<MusicFace>,
    pub status: Vec<MusicFace>,
    /// A book's speed when not as recorded: a word, padded as text is, after the state symbols.
    pub speed: Option<MusicFace>,
    pub progress: Option<f32>,
    /// Bookmarks along the bar, 0 to 1.
    pub marks: Vec<f32>,
    pub place: Option<MusicFace>,
    /// A mark before the place saying whose it is: a book's, where it is the time the book has
    /// left beside the chapter's.
    pub place_mark: Option<MusicFace>,
    pub time: Option<MusicFace>,
    /// The track after this one, under the picture.
    pub next: Option<MusicFace>,
    /// Dynamic Color: the album picture's colour.
    pub tint: Option<MusicTint>,
}

/// Where the screen's animations stand, 0 to 1; the default is at rest.
#[derive(Copy, Clone, Debug)]
pub struct MusicMotion {
    /// The screen coming in, and from which side: 1 deeper, -1 back, 0 in place.
    pub enter: f32,
    pub side: f32,
    pub art: f32,
}

impl Default for MusicMotion {
    fn default() -> Self {
        MusicMotion {
            enter: 1.0,
            side: 0.0,
            art: 1.0,
        }
    }
}

/// The visible width of a line, beside the picture or across the canvas.
pub fn music_text_width(art: bool, canvas: Canvas) -> u32 {
    canvas.w as u32 - QUICK_EDGE as u32 - text_left(art) + 2 * MENU_PAD
}

/// Half the canvas: the most the header's corner takes, and what values at a row's right and
/// Now Playing's time and place take.
pub fn music_half_w(canvas: Canvas) -> u32 {
    canvas.w as u32 / 2
}

/// The title's width beside a corner `corner` wide: what it leaves across the header, a gap
/// between them.
pub fn music_header_title_w(corner: u32, canvas: Canvas) -> u32 {
    let across = canvas.w as u32 - 2 * QUICK_EDGE as u32 + 2 * MENU_PAD;
    let gap = if corner > 0 { CORNER_GAP } else { 0 };
    across.saturating_sub(corner + gap)
}

fn text_left(art: bool) -> u32 {
    if art {
        QUICK_EDGE as u32 + ART_BOX + ART_GAP as u32
    } else {
        QUICK_EDGE as u32
    }
}

pub fn draw_music(faces: &MusicFaces, out: &mut Vec<Draw>) {
    let (canvas, theme) = (faces.canvas, &faces.theme);
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: canvas.w,
        h: OUT_H as f32,
        colour: theme::rgba(theme.ground, 1.0),
    });
    let m = faces.motion;
    if let BodyFaces::NowPlaying(NowPlayingFaces {
        tint: Some(tint), ..
    }) = &faces.body
    {
        tint.draw_wash(canvas, m.art, m.enter, theme, out);
    }
    let pad = MENU_PAD as f32;
    if let Some((tex, w, h)) = faces.title {
        push(out, tex, QUICK_EDGE - pad, TITLE_Y, w, h);
    }
    // A key held to act shows its ring in place of the corner's news.
    let right = canvas.w - QUICK_EDGE + pad;
    let corner_ring = faces
        .ring
        .filter(|(_, hint)| hint.is_none())
        .map(|(face, _)| face);
    match (corner_ring, faces.corner) {
        (Some((tex, w, h)), _) => {
            let y = TITLE_Y + (SMALL_H - h) as f32 / 2.0;
            push(out, tex, right - pad - w as f32, y, w, h);
        }
        (None, Some((tex, w, h))) => push(out, tex, right - w as f32, TITLE_Y, w, h),
        (None, None) => {}
    }
    let body = out.len();
    match &faces.body {
        BodyFaces::List(list) => list::draw(list, canvas, theme, out),
        BodyFaces::NowPlaying(np) => now_playing::draw(np, faces.motion.art, canvas, theme, out),
        BodyFaces::Text(text) => text_view::draw(text, canvas, out),
    }
    // A new body slides in from its side, opaque by halfway so the travel reads; the header
    // and legend stay.
    if m.enter < 1.0 {
        let dx = (m.side * SLIDE_PX * (1.0 - m.enter)).round();
        shift_and_fade(&mut out[body..], dx, (2.0 * m.enter).min(1.0));
    }
    draw_letter(faces, out);
    let hints = centred_hints(canvas, &faces.legend, LEGEND_GAP);
    for &(tex, w, x) in &hints {
        push(out, tex, x, LEGEND_Y, w, HINT_H);
    }
    // A held key's ring, around its cap, which sits at its hint's left.
    if let Some(((tex, w, h), Some(i))) = faces.ring {
        if let Some(&(_, _, x)) = hints.get(i) {
            let gap = face::CAP_RING_GAP as f32;
            let y = LEGEND_Y + (HINT_H - CAP) as f32 / 2.0 - gap;
            push(out, tex, x - gap, y, w, h);
        }
    }
}

/// Moves `draws` across by `dx` and fades them to `alpha`.
fn shift_and_fade(draws: &mut [Draw], dx: f32, alpha: f32) {
    for draw in draws {
        match draw {
            Draw::Rect { x, colour, .. } => (*x, colour[3]) = (*x + dx, colour[3] * alpha),
            Draw::Tex { x, alpha: a, .. } => (*x, *a) = (*x + dx, *a * alpha),
            _ => {}
        }
    }
}

/// A thin line on the theme's track, filled in `ink` as far as `done` (0 to 1).
fn draw_progress(
    x: f32,
    y: f32,
    w: f32,
    done: f32,
    ink: [f32; 4],
    theme: &MusicTheme,
    out: &mut Vec<Draw>,
) {
    out.push(Draw::Rect {
        x,
        y,
        w,
        h: PROGRESS_H,
        colour: theme::rgba(theme.bar, 1.0),
    });
    out.push(Draw::Rect {
        x,
        y,
        w: (w * done.clamp(0.0, 1.0)).round(),
        h: PROGRESS_H,
        colour: ink,
    });
}

/// The letter a list jumped to, in a box in the middle of the rows, as players show it: lit, the
/// letter in the ground's colour, so it never reads as the highlight it may sit beside.
fn draw_letter(faces: &MusicFaces, out: &mut Vec<Draw>) {
    let Some((tex, w, h)) = faces.letter else {
        return;
    };
    let x = faces.canvas.centred(LETTER_BOX);
    let y = ((ROWS_Y + LEGEND_Y - LETTER_BOX) / 2.0).round();
    out.push(Draw::Rect {
        x,
        y,
        w: LETTER_BOX,
        h: LETTER_BOX,
        colour: theme::rgba(faces.theme.lit, 0.95),
    });
    let mid = |side: u32| ((LETTER_BOX - side as f32) / 2.0).round();
    push(out, tex, x + mid(w), y + mid(h), w, h);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_letter_s_box_is_never_the_highlight_s_colour() {
        for theme in [MusicTheme::dark(), MusicTheme::light()] {
            let faces = MusicFaces {
                letter: Some((slot_gfx::TexId::from_raw(1), 40, 40)),
                theme,
                ..MusicFaces::default()
            };
            let mut out = Vec::new();
            draw_letter(&faces, &mut out);
            let Some(Draw::Rect { colour, .. }) = out.first().copied() else {
                panic!("a box");
            };
            assert_ne!(colour, theme::rgba(theme.bar, colour[3]));
        }
    }

    #[test]
    fn the_title_takes_what_the_corner_leaves() {
        let canvas = Canvas::FRAMED;
        let across = music_header_title_w(0, canvas);
        assert!(across > music_half_w(canvas), "an empty corner: across");
        assert_eq!(music_header_title_w(100, canvas), across - 100 - CORNER_GAP);
    }
}
