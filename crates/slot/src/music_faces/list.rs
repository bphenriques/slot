//! A list page's faces: the rows, their values, favorites' hearts, and the highlight. A message
//! is a lone row.

use super::Textures;
use slot_input::Millis;
use slot_music::{AlbumId, Art, ListBody, RowText};
use slot_music_ui::{
    music_half_w, music_number_w, music_rich_px, music_row_ink, music_row_px, music_row_text_w,
    music_text_width, Ink, ListFaces, MusicFace, MUSIC_STATUS,
};
use slot_ui::Canvas;

use super::{Glyphs, MusicText, Spec};

pub(super) fn faces(
    text: &mut MusicText,
    compositor: &mut dyn Textures,
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
    // Two-line rows: the name at a list's usual size, and who made it a size down, dim, below;
    // a menu's lead row, its rows' size and a size down.
    let (row_px, second_px) = if body.two_lines {
        music_rich_px()
    } else {
        (row_px, (row_px as f32 * 0.8).round() as u32)
    };
    let width = music_text_width(false, canvas);
    let hearts: Vec<bool> = body.rows.iter().map(|r| r.favorite).collect();
    let thumbs = thumbs(text, compositor, &body.rows, arts);
    let head = body.rows.iter().take_while(|r| r.action).count();
    // A list's rows past their action rows have thumbnails, and a menu's lead row.
    let thumbed = |i: usize| (body.thumbs && i >= head) || (body.lead && i == 0);
    // What plays is marked on its thumbnail, where it has one, its name kept in line.
    let playing = body
        .rows
        .iter()
        .enumerate()
        .position(|(i, r)| r.playing && thumbed(i));
    let value_w = music_half_w(canvas);
    let mut values = Vec::with_capacity(body.values.len());
    for (i, value) in body.values.into_iter().enumerate() {
        // Dim, lit on the highlighted row, as in the quick menu.
        let spec = Spec::Row {
            text: RowText::plain(value.unwrap_or_default()),
            icon: None,
            ink: if i == selected { Ink::Lit } else { Ink::Dim },
            px: row_px,
        };
        values.push(
            text.values
                .sync(compositor, text.theme, i, spec, value_w, None),
        );
    }
    let mut seconds = Vec::with_capacity(body.rows.len());
    let mut indents = Vec::with_capacity(body.rows.len());
    let mut rows = Vec::with_capacity(body.rows.len());
    for (i, mut row) in body.rows.into_iter().enumerate() {
        let two = body.two_lines || (body.lead && i == 0);
        let second = (two && !row.after.is_empty()).then(|| std::mem::take(&mut row.after));
        // Under the name, past the row's number, as players set a compilation's songs.
        indents.push(if body.two_lines {
            music_number_w(&row.before, row_px as f32)
        } else {
            0
        });
        seconds.push(second.and_then(|after| {
            let spec = Spec::Second(after, second_px);
            let width = music_row_text_w(width, false, 0, thumbed(i));
            text.seconds
                .sync(compositor, text.theme, i, spec, width, None)
        }));
        // Faint rows read dim on the highlight; only it scrolls, or every row of a list that only
        // tells.
        let ink = music_row_ink(row.faint, i == selected);
        let value = values.get(i).copied().flatten().map_or(0, |(_, w, _)| w);
        let width = music_row_text_w(width, row.favorite, value, thumbed(i));
        // What plays is marked before its name where it has no picture to mark it on.
        let icon = (row.playing && !thumbed(i)).then_some(MUSIC_STATUS[usize::from(body.paused)]);
        let spec = Spec::Row {
            text: row,
            icon,
            ink,
            px: row_px,
        };
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
        titled: body.titled,
        per_page: body.per_page,
        head,
        offsets: text.motion.offsets(now),
        rules: body.rules,
        progress: body.progress,
        lifted: body.lifted,
        hearts,
        heart,
        two_lines: body.two_lines,
        actions: body.actions,
        indents,
        thumbed: body.thumbs,
        lead: body.lead,
        thumbs,
        seconds,
        blank: (body.thumbs || body.lead).then_some(glyphs.blank[usize::from(body.books)]),
        playing,
        play: (body.thumbs || body.lead).then_some(glyphs.status[usize::from(body.paused)]),
    }
}

/// The page's thumbnails, each uploaded once as it is read: those gone from the page are let go,
/// their textures kept for the next.
fn thumbs(
    text: &mut MusicText,
    compositor: &mut dyn Textures,
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
    compositor: &mut dyn Textures,
    canvas: Canvas,
    line: String,
    now: Millis,
) -> ListFaces {
    let scroll = text.scroll.ms((None, line.clone()), now);
    let spec = Spec::Row {
        text: RowText::plain(line),
        icon: None,
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
