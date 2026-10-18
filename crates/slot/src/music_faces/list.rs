//! A list page's faces: the rows, their values, favorites' hearts, and the highlight. A message
//! is a lone row.

use slot_gfx::Compositor;
use slot_input::Millis;
use slot_music::{ListBody, RowText};
use slot_ui::{
    music_action_px, music_row_px, music_text_width, music_title_w, Canvas, Ink, ListFaces,
    MUSIC_HEART_ROOM,
};

use super::{Glyphs, MusicText, Spec};

/// Room left between a row's text and its value.
const VALUE_GAP: u32 = 20;

pub(super) fn faces(
    text: &mut MusicText,
    compositor: &mut Compositor,
    canvas: Canvas,
    body: ListBody,
    glyphs: Glyphs,
    now: Millis,
) -> ListFaces {
    let selected = body.selected;
    let focus = body.rows.get(selected).map(|r| r.text.clone());
    let scroll = text
        .scroll
        .ms((Some(selected), focus.unwrap_or_default()), now);
    let bar = text.motion.bar(&body.rows, selected, now);
    let row_px = music_row_px(body.menu, body.per_page) as u32;
    let action_px = music_action_px(body.menu, body.per_page) as u32;
    let width = music_text_width(false, canvas);
    let hearts: Vec<bool> = body.rows.iter().map(|r| r.favorite).collect();
    let opens: Vec<bool> = body.rows.iter().map(|r| r.opens).collect();
    let head = body.rows.iter().take_while(|r| r.action).count();
    let value_w = music_title_w(false, canvas);
    let mut values = Vec::with_capacity(body.values.len());
    for (i, value) in body.values.into_iter().enumerate() {
        // Dim, lit on the highlighted row, as in the quick menu.
        let spec = Spec::Row {
            text: RowText::plain(value.unwrap_or_default()),
            ink: if i == selected { Ink::Lit } else { Ink::Dim },
            px: row_px,
        };
        values.push(
            text.values
                .sync(compositor, text.theme, i, spec, value_w, None),
        );
    }
    let mut rows = Vec::with_capacity(body.rows.len());
    for (i, row) in body.rows.into_iter().enumerate() {
        // Action rows are a size down. Faint rows read dim on the highlight; only it scrolls,
        // or every row of a list that only tells.
        let px = if row.action { action_px } else { row_px };
        let ink = match (row.faint, i == selected) {
            (true, false) => Ink::Faint,
            (true, true) => Ink::Dim,
            (false, _) => Ink::Lit,
        };
        // Text stops short of its heart or ›, and of its value: a long chapter of its length.
        let value = values.get(i).copied().flatten().filter(|v| v.1 > 0);
        let value = value.map_or(0, |(_, w, _)| w + VALUE_GAP);
        let marked = u32::from(row.favorite || row.opens);
        let width = width - marked * MUSIC_HEART_ROOM - value;
        let spec = Spec::Row { text: row, ink, px };
        let scroll = (body.readonly || i == selected).then_some(scroll);
        rows.push(
            text.lines
                .sync(compositor, text.theme, i, spec, width, scroll),
        );
    }
    let heart = hearts
        .contains(&true)
        .then(|| text.row_heart(compositor, body.menu, body.per_page));
    ListFaces {
        rows,
        values,
        carets: body.adjustable.then_some(glyphs.carets),
        selected: (!body.readonly).then_some(selected),
        bar: (!body.readonly).then_some(bar),
        menu: body.menu,
        per_page: body.per_page,
        head,
        offsets: text.motion.offsets(now),
        rules: body.rules,
        progress: body.progress,
        lifted: body.lifted,
        hearts,
        heart,
        // The settings' right caret, dim: a page opens on the right.
        arrow: opens.contains(&true).then_some(glyphs.carets[1]),
        opens,
    }
}

/// A line in place of a list, set as a menu row in the middle, scrolling if it is long.
pub(super) fn message(
    text: &mut MusicText,
    compositor: &mut Compositor,
    canvas: Canvas,
    line: String,
    now: Millis,
) -> ListFaces {
    let scroll = text.scroll.ms((None, line.clone()), now);
    let spec = Spec::Row {
        text: RowText::plain(line),
        ink: Ink::Lit,
        px: music_row_px(true, 1) as u32,
    };
    let row = text.lines.sync(
        compositor,
        text.theme,
        0,
        spec,
        music_text_width(false, canvas),
        Some(scroll),
    );
    ListFaces {
        rows: vec![row],
        menu: true,
        per_page: 1,
        ..ListFaces::default()
    }
}
