//! A list page: the highlight, the rows, rules under the action rows and between discs, and
//! values and favorites' hearts at the right as in the quick menu. Menus sit in the middle of
//! the screen as slot's do; lists fill the room under the header.

use super::slot::{push, Canvas, Draw, MENU_PAD, MENU_PX, QUICK_EDGE, QUICK_PITCH};
use super::theme::rgba;
use super::{
    draw_progress, Ink, ListFaces, MusicFace, MusicTheme, ACTION_SCALE, BAR_INSET, CARET_GAP,
    LEGEND_Y, LIST_TYPE_PER_ROW, MUSIC_HEART_ROOM, PROGRESS_Y, ROWS_Y,
};

/// The type size of a page's rows: the quick menu's for menus, from the spacing for lists.
pub fn music_row_px(menu: bool, per_page: usize) -> f32 {
    if menu {
        MENU_PX
    } else {
        (Grid::new(menu, per_page).pitch * LIST_TYPE_PER_ROW).round()
    }
}

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
/// `value_w` wide, if any.
pub fn music_row_text_w(width: u32, heart: bool, value_w: u32) -> u32 {
    let value = if value_w > 0 { value_w + VALUE_GAP } else { 0 };
    width.saturating_sub(u32::from(heart) * MUSIC_HEART_ROOM + value)
}

/// Action rows are a size down.
pub fn music_action_px(menu: bool, per_page: usize) -> f32 {
    (music_row_px(menu, per_page) * ACTION_SCALE).round()
}

/// Where a page's rows go: the first one's top, the spacing, how many action rows lead, and how
/// far rows are from their places as they slide there.
#[derive(Copy, Clone)]
struct Grid<'a> {
    top: f32,
    pitch: f32,
    head: usize,
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
                offsets: &[],
            }
        } else {
            Grid {
                top: ROWS_Y,
                pitch: ((LEGEND_Y - 8.0 - ROWS_Y) / rows).floor(),
                head: 0,
                offsets: &[],
            }
        }
    }

    /// The page `list` lays out. Action rows are set a size down and as much closer; the room
    /// saved goes above the rule under them, so the rows after keep their places.
    fn of(list: &'a ListFaces) -> Grid<'a> {
        Grid {
            head: list.head,
            offsets: &list.offsets,
            ..Grid::new(list.menu, list.per_page)
        }
    }

    fn height(self, i: usize) -> f32 {
        if i < self.head {
            self.pitch * ACTION_SCALE
        } else {
            self.pitch
        }
    }

    /// The top of row `i`, or between two rows while the highlight glides.
    fn row_top(self, i: f32) -> f32 {
        let head = self.head as f32;
        let action = |i: f32| self.top + i * self.pitch * ACTION_SCALE;
        if i <= head - 1.0 {
            action(i)
        } else if i < head {
            let (from, to) = (action(head - 1.0), self.top + head * self.pitch);
            from + (to - from) * (i - (head - 1.0))
        } else {
            self.top + i * self.pitch
        }
    }

    /// The top of a face `h` tall, centred in row `i`, or on its way there.
    fn row_mid(self, i: usize, h: u32) -> f32 {
        let sliding = self.offsets.get(i).copied().unwrap_or(0.0) * self.pitch;
        self.row_top(i as f32) + (self.height(i) - h as f32) / 2.0 + sliding
    }

    /// A rule above row `i`: midway between it and the end of the row before.
    fn rule_y(self, i: usize) -> f32 {
        let top = self.row_top(i as f32);
        match i.checked_sub(1) {
            Some(above) => (self.row_top(above as f32) + self.height(above) + top) / 2.0,
            None => top,
        }
    }
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
    let row = |i: usize, out: &mut Vec<Draw>| {
        if let Some((tex, w, h)) = list.rows[i] {
            push(out, tex, QUICK_EDGE - pad, grid.row_mid(i, h), w, h);
        }
    };
    for i in (0..list.rows.len()).filter(|&i| Some(i) != card) {
        row(i, out);
    }
    if let Some(i) = card {
        draw_bar(list, grid, canvas, theme, out);
        row(i, out);
    }
    let right = canvas.w - QUICK_EDGE;
    draw_rules(list, grid, right, theme, out);
    draw_values(list, grid, right, out);
    draw_marks(list, &list.hearts, list.heart, grid, right, theme, out);
}

/// The highlight, where it is gliding to; lighter when its row is lifted.
fn draw_bar(list: &ListFaces, grid: Grid, canvas: Canvas, theme: &MusicTheme, out: &mut Vec<Draw>) {
    let Some(at) = list.bar else {
        return;
    };
    out.push(Draw::Rect {
        x: 0.0,
        y: grid.row_top(at).round() + BAR_INSET,
        w: canvas.w,
        h: grid.height(at.round() as usize) - 2.0 * BAR_INSET,
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
    use crate::quick_menu::{quick_row_top, QuickRow};

    #[test]
    fn a_row_is_inked_by_its_state_and_its_text_stops_short_of_its_heart_and_value() {
        assert_eq!(music_row_ink(true, false), Ink::Faint);
        assert_eq!(
            music_row_ink(true, true),
            Ink::Dim,
            "faint reads dim on the highlight"
        );
        assert_eq!(music_row_ink(false, true), Ink::Lit);
        assert_eq!(music_row_text_w(500, false, 0), 500);
        assert_eq!(music_row_text_w(500, true, 0), 500 - MUSIC_HEART_ROOM);
        assert_eq!(music_row_text_w(500, false, 60), 500 - 60 - VALUE_GAP);
    }

    #[test]
    fn lists_scale_from_normal_and_menus_keep_the_quick_menu_size() {
        assert_eq!(music_row_px(true, 6), MENU_PX, "a menu");
        assert_eq!(music_row_px(false, 8), 24.0, "Normal");
        assert!(music_row_px(false, 10) < 24.0 && music_row_px(false, 7) > 24.0);
        assert_eq!(Grid::new(true, 6).pitch, QUICK_PITCH);
    }

    #[test]
    fn a_menu_sits_where_the_quick_menu_would() {
        let first = QuickRow::MAIN[0];
        assert_eq!(
            Grid::new(true, QuickRow::MAIN.len()).top,
            quick_row_top(first, &[]).round()
        );
    }
}
