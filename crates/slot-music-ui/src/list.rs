//! A list page: the highlight, the rows, rules under the action rows and between discs, and
//! values and favorites' hearts at the right as in the quick menu. Menus sit in the middle of
//! the screen as slot's do; lists fill the room under the header.

use super::chrome::{push, Canvas, Draw, MENU_PAD, MENU_PX, QUICK_EDGE, QUICK_PITCH};
use super::face::ROW_FACE_PAD;
use super::theme::rgba;
use super::{
    draw_progress, Ink, ListFaces, MusicFace, MusicTheme, BAR_INSET, CARET_GAP, LEGEND_Y,
    LIST_TYPE_PER_ROW, MUSIC_HEART_ROOM, PROGRESS_Y, ROWS_Y, THUMB_PX, THUMB_ROOM,
};

/// The type size of a page's rows: the quick menu's for menus, from the spacing for lists.
pub fn music_row_px(menu: bool, per_page: usize) -> f32 {
    if menu {
        MENU_PX
    } else {
        (Grid::new(menu, per_page).pitch * LIST_TYPE_PER_ROW).round()
    }
}

/// The type of a two-line row: its name at a list's usual size, eight a page, and who made it a
/// size down below.
pub fn music_rich_px() -> (u32, u32) {
    let title = music_row_px(false, 8);
    (title as u32, (title * 0.8).round() as u32)
}

/// Rows on a page of a one-line list, whose height an action row keeps in every list.
const ONE_LINE_ROWS: f32 = 8.0;

/// The gap between a two-line row's lines, their type to type.
const LINE_GAP: f32 = 4.0;

/// How far a two-line row's lines are lifted, of the name's type, so the pair is centred from the
/// top of the name's capitals to the baseline under it, as type is set: a letter hanging below,
/// as a p does, dips into the room beneath rather than pushing the pair up.
const CAPS_LIFT: f32 = 0.04;

/// Room left between a row's text and its value.
const VALUE_GAP: u32 = 20;

/// A row's ink: faint when behind or unavailable, dim so on the highlight; else lit.
pub fn music_row_ink(faint: bool, selected: bool) -> Ink {
    match (faint, selected) {
        (true, false) => Ink::Faint,
        (true, true) => Ink::Dim,
        (false, _) => Ink::Lit,
    }
}

/// The room a row's text has of `width`: short of its heart, if it has one, and of a value
/// `value_w` wide, if any., and of a
/// thumbnail before it in a list that has them.
pub fn music_row_text_w(width: u32, heart: bool, value_w: u32, thumbed: bool) -> u32 {
    let value = if value_w > 0 { value_w + VALUE_GAP } else { 0 };
    let thumb = if thumbed { THUMB_ROOM as u32 } else { 0 };
    width.saturating_sub(u32::from(heart) * MUSIC_HEART_ROOM + value + thumb)
}

/// How strong a thumbnail's empty square is, of the dim ink: there until it is read, or for an
/// album with no picture.
const THUMB_BLANK: f32 = 0.15;

/// Where a page's rows go: the first one's top, the spacing, how many action rows lead it
/// (Play All) and their spacing, a one-line row's in every list so they look the same in each,
/// and how far rows are from their places as they slide there.
#[derive(Copy, Clone)]
struct Grid<'a> {
    top: f32,
    pitch: f32,
    head: usize,
    head_pitch: f32,
    offsets: &'a [f32],
}

impl<'a> Grid<'a> {
    /// Menus keep the quick menu's spacing, centred as its rows are; lists share the room
    /// between the header and the hints.
    fn new(menu: bool, per_page: usize) -> Grid<'a> {
        let rows = per_page.max(1) as f32;
        if menu {
            Grid {
                top: ((LEGEND_Y - QUICK_PITCH * rows) / 2.0).round(),
                pitch: QUICK_PITCH,
                head: 0,
                head_pitch: QUICK_PITCH,
                offsets: &[],
            }
        } else {
            Grid {
                top: ROWS_Y,
                pitch: ((LEGEND_Y - 8.0 - ROWS_Y) / rows).floor(),
                head: 0,
                head_pitch: ((LEGEND_Y - 8.0 - ROWS_Y) / ONE_LINE_ROWS).floor(),
                offsets: &[],
            }
        }
    }

    /// The spacing of `per_page` two-line rows under `actions` one-line action rows.
    fn rich_pitch(per_page: usize, actions: usize) -> f32 {
        let page = Grid::new(false, ONE_LINE_ROWS as usize);
        let room = LEGEND_Y - 8.0 - ROWS_Y - actions as f32 * page.head_pitch;
        (room / per_page.max(1) as f32).floor()
    }

    /// Where a full page of a list ends.
    fn list_end() -> f32 {
        let page = Grid::new(false, ONE_LINE_ROWS as usize);
        page.top + ONE_LINE_ROWS * page.pitch
    }

    /// The page `list` lays out.
    fn of(list: &'a ListFaces) -> Grid<'a> {
        let grid = Grid::new(list.menu, list.per_page);
        // Two-line rows leave the first page room for its action row (Play All), and are as tall
        // on every page, so the rows look alike on each.
        let pitch = if list.two_lines {
            Grid::rich_pitch(list.per_page, list.actions)
        } else {
            grid.pitch
        };
        Grid {
            // A titled menu starts at the top, as a list does, however short; a long one higher,
            // so it ends no nearer the hints than a list's page.
            top: if list.menu && list.titled {
                ROWS_Y.min(Grid::list_end() - grid.pitch * list.per_page.max(1) as f32)
            } else {
                grid.top
            },
            pitch,
            head: list.head,
            offsets: &list.offsets,
            ..grid
        }
    }

    /// How tall row `i` is: an action row a one-line row's height, the rest the page's.
    fn height(self, i: usize) -> f32 {
        if i < self.head {
            self.head_pitch
        } else {
            self.pitch
        }
    }

    /// The top of row `i`, or between two rows while the highlight glides.
    fn row_top(self, i: f32) -> f32 {
        let head = self.head as f32;
        let past = (i - head).max(0.0);
        self.top + i.min(head) * self.head_pitch + past * self.pitch
    }

    /// The top of a face `h` tall, centred in row `i`, or on its way there.
    fn row_mid(self, i: usize, h: u32) -> f32 {
        let sliding = self.offsets.get(i).copied().unwrap_or(0.0) * self.pitch;
        self.row_top(i as f32) + (self.height(i) - h as f32) / 2.0 + sliding
    }

    /// A rule above row `i`: where its line starts, the line before ending there.
    fn rule_y(self, i: usize) -> f32 {
        self.row_top(i as f32)
    }
}

/// Where a menu of `rows` starts down the screen: centred above the hints, at slot's quick
/// menu's pitch, so a music menu sits where the quick menu would.
pub fn music_menu_top(rows: usize) -> f32 {
    Grid::new(true, rows).top
}

pub(super) fn draw(list: &ListFaces, canvas: Canvas, theme: &MusicTheme, out: &mut Vec<Draw>) {
    let grid = Grid::of(list);
    // A lifted row is a card above the others: they slide under it as it is carried.
    let card = list.selected.filter(|_| list.lifted);
    if card.is_none() {
        draw_bar(list, grid, canvas, theme, out);
    }
    if let Some(done) = list.progress {
        let w = canvas.w - 2.0 * QUICK_EDGE;
        let ink = rgba(theme.lit, 1.0);
        draw_progress(QUICK_EDGE, PROGRESS_Y, w, done, ink, theme, out);
    }
    let pad = MENU_PAD as f32;
    // A list's rows past their action rows have thumbnails.
    let thumbed = |i: usize| list.thumbed && i >= list.head;
    let row = |i: usize, out: &mut Vec<Draw>| {
        if thumbed(i) {
            draw_thumb(list, grid, i, theme, out);
        }
        // Past the thumbnail, if any.
        let x = QUICK_EDGE - pad + if thumbed(i) { THUMB_ROOM } else { 0.0 };
        let second = list.seconds.get(i).copied().flatten();
        match (list.rows[i], second) {
            // Two lines, their type centred together in the row, the blank each face keeps above
            // and below left out: the name, then who made it below.
            (Some((tex, w, h)), Some((stex, sw, sh))) => {
                let (name, by) = (
                    h as f32 - 2.0 * ROW_FACE_PAD,
                    sh as f32 - 2.0 * ROW_FACE_PAD,
                );
                let top = grid.row_mid(i, (name + LINE_GAP + by).round() as u32)
                    - (name * CAPS_LIFT).round();
                push(out, tex, x, top - ROW_FACE_PAD, w, h);
                let indent = list.indents.get(i).copied().unwrap_or(0) as f32;
                push(
                    out,
                    stex,
                    x + indent,
                    top + name + LINE_GAP - ROW_FACE_PAD,
                    sw,
                    sh,
                );
            }
            (Some((tex, w, h)), None) => push(out, tex, x, grid.row_mid(i, h), w, h),
            _ => {}
        }
    };
    let rest = out.len();
    for i in (0..list.rows.len()).filter(|&i| Some(i) != card) {
        row(i, out);
    }
    let right = canvas.w - QUICK_EDGE;
    draw_rules(list, grid, right, theme, out);
    draw_values(list, grid, right, out);
    draw_marks(list, &list.hearts, list.heart, grid, right, theme, out);
    // A row carried is lifted above the rest, which fade back under it, so it is the one seen.
    if let Some(i) = card {
        super::shift_and_fade(&mut out[rest..], 0.0, theme.behind);
        draw_bar(list, grid, canvas, theme, out);
        row(i, out);
    }
}

/// Row `i`'s album thumbnail at its start, centred in its box; until it is read, or with no
/// picture, an empty square with a note or a book on it, as Now Playing's square has.
fn draw_thumb(list: &ListFaces, grid: Grid, i: usize, theme: &MusicTheme, out: &mut Vec<Draw>) {
    let face = list.thumbs.get(i).copied().flatten();
    if face.is_none() {
        out.push(Draw::Rect {
            x: QUICK_EDGE,
            y: grid.row_mid(i, THUMB_PX),
            w: THUMB_PX as f32,
            h: THUMB_PX as f32,
            colour: rgba(theme.dim, THUMB_BLANK),
        });
    }
    if let Some((tex, w, h)) = face.or(list.blank) {
        let x = QUICK_EDGE + (THUMB_PX - w) as f32 / 2.0;
        push(out, tex, x, grid.row_mid(i, h), w, h);
    }
    // What plays: its thumbnail shaded, and the play mark on it.
    if let (Some((tex, w, h)), true) = (list.play, list.playing == Some(i)) {
        let top = grid.row_mid(i, THUMB_PX);
        out.push(Draw::Rect {
            x: QUICK_EDGE,
            y: top,
            w: THUMB_PX as f32,
            h: THUMB_PX as f32,
            colour: rgba(theme.ground, THUMB_SHADE),
        });
        let (x, y) = (
            QUICK_EDGE + (THUMB_PX - w) as f32 / 2.0,
            top + (THUMB_PX - h) as f32 / 2.0,
        );
        push(out, tex, x, y, w, h);
    }
}

/// How strongly the playing row's thumbnail is shaded under its play mark, of the ground.
const THUMB_SHADE: f32 = 0.55;

/// The highlight, where it is gliding to; lighter when its row is lifted.
fn draw_bar(list: &ListFaces, grid: Grid, canvas: Canvas, theme: &MusicTheme, out: &mut Vec<Draw>) {
    let Some(at) = list.bar else {
        return;
    };
    // From its row's top to the next one's: between a one-line and a two-line row, as tall as
    // the way it has come, so it grows as it glides rather than at once.
    let (top, bottom) = (grid.row_top(at).round(), grid.row_top(at + 1.0).round());
    out.push(Draw::Rect {
        x: 0.0,
        y: top + BAR_INSET,
        w: canvas.w,
        h: bottom - top - 2.0 * BAR_INSET,
        colour: if list.lifted {
            lifted(theme)
        } else {
            rgba(theme.bar, 1.0)
        },
    });
}

/// The highlight of a lifted row: the bar a share of the way to the ink, as if raised.
fn lifted(theme: &MusicTheme) -> [f32; 4] {
    let (bar, ink) = (rgba(theme.bar, 1.0), rgba(theme.lit, 1.0));
    [0, 1, 2, 3].map(|i| bar[i] + (ink[i] - bar[i]) * theme.lift)
}

/// Thin rules above the rows that start a part: after the action rows, a new disc.
fn draw_rules(list: &ListFaces, grid: Grid, right: f32, theme: &MusicTheme, out: &mut Vec<Draw>) {
    for &row in list.rules.iter().filter(|&&r| r < list.rows.len()) {
        out.push(Draw::Rect {
            x: QUICK_EDGE,
            y: grid.rule_y(row).round(),
            w: right - QUICK_EDGE,
            h: 1.0,
            colour: rgba(theme.dim, 0.5),
        });
    }
}

/// Values at the right edge, with carets round the highlighted one when they change.
fn draw_values(list: &ListFaces, grid: Grid, right: f32, out: &mut Vec<Draw>) {
    let pad = MENU_PAD as f32;
    for (i, value) in list.values.iter().enumerate() {
        let Some((tex, w, h)) = *value else {
            continue;
        };
        let y = grid.row_mid(i, h);
        match (list.carets, list.selected == Some(i)) {
            (Some([(lt, lw, lh), (rt, rw, rh)]), true) => {
                let rx = right - rw as f32;
                push(out, rt, rx, grid.row_mid(i, rh), rw, rh);
                let vx = rx - CARET_GAP + pad - w as f32;
                push(out, tex, vx, y, w, h);
                let lx = vx + pad - CARET_GAP - lw as f32;
                push(out, lt, lx, grid.row_mid(i, lh), lw, lh);
            }
            _ => push(out, tex, right + pad - w as f32, y, w, h),
        }
    }
}

/// A mark at the right edge of the rows `on` says: a favorite's heart. Dim, as values are, and
/// lit on the highlight.
fn draw_marks(
    list: &ListFaces,
    on: &[bool],
    mark: Option<MusicFace>,
    grid: Grid,
    right: f32,
    theme: &MusicTheme,
    out: &mut Vec<Draw>,
) {
    let Some((tex, w, h)) = mark else {
        return;
    };
    let x = right - w as f32;
    for (i, _) in on.iter().enumerate().filter(|(_, &on)| on) {
        push(out, tex, x, grid.row_mid(i, h), w, h);
        if let Some(Draw::Tex { alpha, .. }) = out.last_mut() {
            *alpha = if list.selected == Some(i) {
                1.0
            } else {
                theme.heart_dim
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TexId;

    #[test]
    fn a_row_is_inked_by_its_state_and_its_text_stops_short_of_its_heart_and_value() {
        assert_eq!(music_row_ink(true, false), Ink::Faint);
        assert_eq!(
            music_row_ink(true, true),
            Ink::Dim,
            "faint reads dim on the highlight"
        );
        assert_eq!(music_row_ink(false, true), Ink::Lit);
        assert_eq!(music_row_text_w(500, false, 0, false), 500);
        assert_eq!(
            music_row_text_w(500, true, 0, false),
            500 - MUSIC_HEART_ROOM
        );
        assert_eq!(
            music_row_text_w(500, false, 60, false),
            500 - 60 - VALUE_GAP
        );
        assert_eq!(
            music_row_text_w(500, false, 0, true),
            500 - THUMB_ROOM as u32
        );
    }

    #[test]
    fn a_thumbnail_and_two_lines_fit_inside_a_two_line_row_s_highlight() {
        let row = Grid::rich_pitch(5, 1) - 2.0 * BAR_INSET;
        let (title, second) = music_rich_px();
        // The room round their ink is held on the pixels, in the screenshots' tests.
        assert!(
            (title + second) as f32 + LINE_GAP <= row,
            "two lines in {row}"
        );
        assert!(THUMB_PX as f32 <= row, "{THUMB_PX} in {row}");
    }

    #[test]
    fn an_empty_thumbnail_shows_its_note_or_book_and_a_read_one_its_picture() {
        let (blank, picture) = (TexId(1), TexId(2));
        let list = ListFaces {
            two_lines: true,
            thumbed: true,
            thumbs: vec![None, Some((picture, THUMB_PX, THUMB_PX))],
            blank: Some((blank, 22, 22)),
            ..ListFaces::default()
        };
        let drawn = |i| {
            let mut out = Vec::new();
            draw_thumb(&list, Grid::new(false, 5), i, &MusicTheme::dark(), &mut out);
            out.iter()
                .filter_map(|d| match d {
                    Draw::Tex { tex, .. } => Some(*tex),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(drawn(0), [blank], "on its empty square");
        assert_eq!(drawn(1), [picture], "the picture alone");
    }

    #[test]
    fn a_carried_row_stays_whole_and_the_rows_behind_it_fade() {
        let face = |i| Some((TexId(i), 100, 30));
        let list = ListFaces {
            rows: vec![face(1), face(2), face(3)],
            selected: Some(1),
            bar: Some(1.0),
            lifted: true,
            per_page: 8,
            ..ListFaces::default()
        };
        let mut out = Vec::new();
        draw(&list, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
        let alpha = |id| {
            out.iter().find_map(|d| match *d {
                Draw::Tex { tex, alpha, .. } if tex == TexId(id) => Some(alpha),
                _ => None,
            })
        };
        assert_eq!(alpha(2), Some(1.0), "the carried row");
        let behind = Some(MusicTheme::dark().behind);
        assert_eq!(
            (alpha(1), alpha(3)),
            (behind, behind),
            "the rest, faded back"
        );
    }

    #[test]
    fn a_two_line_list_with_no_play_all_fits_a_row_more_in_its_room() {
        let list = |actions, per_page| ListFaces {
            two_lines: true,
            thumbed: true,
            actions,
            per_page,
            ..ListFaces::default()
        };
        let (albums, queue) = (list(1, 5), list(0, 6));
        let (albums, queue) = (Grid::of(&albums), Grid::of(&queue));
        assert!(queue.row_top(6.0) <= LEGEND_Y - 8.0, "six above the hints");
        assert!(
            queue.pitch >= albums.pitch - 5.0,
            "nearly as roomy: {}",
            queue.pitch
        );
    }

    #[test]
    fn play_all_is_as_tall_in_a_two_line_list_as_in_a_one_line_list_and_a_full_page_under_it() {
        let list = |head| ListFaces {
            two_lines: true,
            thumbed: true,
            head,
            actions: 1,
            per_page: 5,
            ..ListFaces::default()
        };
        let (first, next) = (list(1), list(0));
        let (one, two, later) = (Grid::new(false, 8), Grid::of(&first), Grid::of(&next));
        assert_eq!(two.pitch, later.pitch, "two-line rows alike on every page");
        assert!(
            two.row_top(6.0) <= LEGEND_Y - 8.0,
            "Play All and five albums above the hints"
        );
        assert_eq!(two.height(0), one.pitch, "Play All, a one-line row");
        assert_eq!(two.height(1), two.pitch, "the albums, two lines");
        assert_eq!(
            two.row_top(1.0),
            two.top + one.pitch,
            "the first album right below"
        );
    }

    #[test]
    fn the_highlight_grows_as_it_glides_from_play_all_to_a_two_line_row() {
        let list = ListFaces {
            two_lines: true,
            thumbed: true,
            head: 1,
            per_page: 5,
            bar: Some(0.5),
            ..ListFaces::default()
        };
        let grid = Grid::of(&list);
        let mut out = Vec::new();
        draw_bar(&list, grid, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
        let Some(Draw::Rect { h, .. }) = out.first().copied() else {
            panic!("a bar");
        };
        let (one, two) = (grid.height(0), grid.height(1));
        assert!(
            h + 2.0 * BAR_INSET > one && h + 2.0 * BAR_INSET < two,
            "{h} between"
        );
    }

    #[test]
    fn lists_scale_from_normal_and_menus_keep_the_quick_menu_size() {
        assert_eq!(music_row_px(true, 6), MENU_PX, "a menu");
        assert_eq!(music_row_px(false, 8), 24.0, "Normal");
        assert!(music_row_px(false, 10) < 24.0 && music_row_px(false, 7) > 24.0);
        assert_eq!(Grid::new(true, 6).pitch, QUICK_PITCH);
    }

    #[test]
    fn a_titled_menu_starts_where_a_list_does_and_ends_no_nearer_the_hints() {
        let menu = |rows| ListFaces {
            menu: true,
            titled: true,
            per_page: rows,
            ..ListFaces::default()
        };
        let short = menu(3);
        assert_eq!(Grid::of(&short).top, ROWS_Y, "at the top, not centred");
        let long = menu(7);
        let grid = Grid::of(&long);
        assert!(
            grid.top + 7.0 * grid.pitch <= Grid::list_end(),
            "above the hints"
        );
        assert!(Grid::list_end() <= LEGEND_Y - 8.0);
    }
}
