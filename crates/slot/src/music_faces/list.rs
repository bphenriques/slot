//! A list page's faces: the rows, their values, favorites' hearts, and the highlight. A message
//! is a lone row.

use slot_gfx::Compositor;
use slot_input::Millis;
use slot_music::{AlbumId, Art, ListBody, RowText};
use slot_ui::music::{
    music_action_px, music_half_w, music_row_ink, music_row_px, music_row_text_w, music_text_width,
    Ink, ListFaces, MusicFace,
};
use slot_ui::Canvas;

use super::{Glyphs, MusicText, Spec};

pub(super) fn faces(
    text: &mut MusicText,
    compositor: &mut Compositor,
    canvas: Canvas,
    body: ListBody,
    glyphs: Glyphs,
    arts: &[Art],
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
    let thumbs = thumbs(text, compositor, &body.rows, arts);
    let head = body.rows.iter().take_while(|r| r.action).count();
    let value_w = music_half_w(canvas);
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
        let ink = music_row_ink(row.faint, i == selected);
        let value = values.get(i).copied().flatten().map_or(0, |(_, w, _)| w);
        let width = music_row_text_w(width, row.favorite, value, body.thumbs);
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
        thumbed: body.thumbs,
        thumbs,
    }
}

/// The page's thumbnails, each uploaded once as it is read: those gone from the page are let go,
/// their textures kept for the next.
fn thumbs(
    text: &mut MusicText,
    compositor: &mut Compositor,
    rows: &[RowText],
    arts: &[Art],
) -> Vec<Option<MusicFace>> {
    let on: Vec<Option<AlbumId>> = rows.iter().map(|r| r.album).collect();
    let (kept, free) = (&mut text.thumbs, &mut text.thumb_free);
    kept.retain(|(album, face)| {
        let shown = on.contains(&Some(*album));
        if !shown {
            free.push(face.0);
        }
        shown
    });
    for art in arts {
        let new = on.contains(&Some(art.album)) && !kept.iter().any(|(a, _)| *a == art.album);
        if new {
            let mut tex = free.pop();
            let id = super::put(compositor, &mut tex, art.w, art.h, &art.rgba);
            kept.push((art.album, (id, art.w, art.h)));
        }
    }
    on.iter()
        .map(|album| {
            let album = (*album)?;
            kept.iter()
                .find(|(a, _)| *a == album)
                .map(|(_, face)| *face)
        })
        .collect()
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
