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
    // The marks after the first centred on its middle, a word (a book's speed) as an icon is.
    let mut middle = bar - NP_BAR_GAP;
    if let Some(&(tex, w, h)) = np.status.first() {
        push(out, tex, left, icons(h), w, h);
        middle = icons(h) + h as f32 / 2.0;
    }
    // A book's speed ends at the margin by its ink, its padding past it, as a line of text does.
    let mut x = right;
    if let Some((tex, w, h)) = np.speed {
        let at = right + pad - w as f32;
        push(out, tex, at, (middle - h as f32 / 2.0).round(), w, h);
        x = at + pad - STATUS_GAP;
    }
    for &(tex, w, h) in np.status.iter().skip(1).rev() {
        x -= w as f32;
        push(out, tex, x, (middle - h as f32 / 2.0).round(), w, h);
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
        let x = right + pad - w as f32;
        push(out, tex, x, small, w, h);
        // Its mark a gap before its ink, centred on it.
        if let Some((mtex, mw, mh)) = np.place_mark {
            let y = (small + (h as f32 - mh as f32) / 2.0).round();
            push(out, mtex, x + pad - STATUS_GAP - mw as f32, y, mw, mh);
        }
    }
    // Up Next, midway between the picture and the hints.
    if let Some((tex, w, h)) = np.next {
        let y = ((NP_TOP + ART_BOX as f32 + LEGEND_Y - h as f32) / 2.0).round();
        push(out, tex, canvas.centred(w as f32).round(), y, w, h);
    }
}

/// The picture centred in its box, fading in over the last one. The empty box, a note or a book
/// on it, shows only before the first picture: between two, it would flash grey round a
/// picture that is not square, so a last picture of another shape fades out instead.
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
        if let Some((tex, w, h)) = np.no_art {
            let x = QUICK_EDGE + (ART_BOX - w) as f32 / 2.0;
            let y = NP_TOP + (ART_BOX - h) as f32 / 2.0;
            push(out, tex, x, y, w, h);
            if let Some(Draw::Tex { alpha, .. }) = out.last_mut() {
                *alpha = 1.0 - shown;
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use slot_gfx::TexId;

    #[test]
    fn a_book_s_speed_ends_at_the_margin_by_its_ink_and_the_heart_sits_a_gap_from_it() {
        let (play, heart, speed) = (TexId::from_raw(1), TexId::from_raw(2), TexId::from_raw(3));
        let np = NowPlayingFaces {
            status: vec![(play, 20, 20), (heart, 20, 20)],
            speed: Some((speed, 80, 30)),
            ..NowPlayingFaces::default()
        };
        let mut out = Vec::new();
        draw(&np, 0.0, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
        let at = |id| {
            out.iter()
                .find_map(|d| match *d {
                    Draw::Tex { tex, x, w, .. } if tex == id => Some((x, w)),
                    _ => None,
                })
                .expect("drawn")
        };
        let pad = MENU_PAD as f32;
        let right = Canvas::FRAMED.w - QUICK_EDGE;
        let (sx, sw) = at(speed);
        assert_eq!(sx + sw - pad, right, "its ink at the margin");
        let (mx, mw) = at(heart);
        assert_eq!(mx + mw, sx + pad - STATUS_GAP, "a gap from its ink");
    }

    #[test]
    fn a_book_s_time_left_has_its_mark_just_before_it_and_clear_of_the_chapter_s_time() {
        let (time, place, mark) = (TexId::from_raw(1), TexId::from_raw(2), TexId::from_raw(3));
        let pad = MENU_PAD;
        let np = NowPlayingFaces {
            time: Some((time, 110 + 2 * pad, 20)),
            place: Some((place, 150 + 2 * pad, 20)),
            place_mark: Some((mark, 22, 22)),
            ..NowPlayingFaces::default()
        };
        let mut out = Vec::new();
        draw(&np, 0.0, Canvas::FRAMED, &MusicTheme::dark(), &mut out);
        let at = |id| {
            out.iter()
                .find_map(|d| match *d {
                    Draw::Tex { tex, x, w, .. } if tex == id => Some((x, x + w)),
                    _ => None,
                })
                .expect("drawn")
        };
        let pad = pad as f32;
        let ((_, time_end), (place_start, _), (mark_start, mark_end)) =
            (at(time), at(place), at(mark));
        assert_eq!(
            mark_end,
            place_start + pad - STATUS_GAP,
            "a gap before its ink"
        );
        assert!(mark_start > time_end - pad, "clear of the chapter's time");
    }

    #[test]
    fn the_empty_square_shows_a_note_until_a_picture_fades_in_over_it() {
        let (note, picture) = (TexId::from_raw(1), TexId::from_raw(2));
        let theme = MusicTheme::dark();
        let np = |art| NowPlayingFaces {
            no_art: Some((note, 96, 96)),
            art,
            ..NowPlayingFaces::default()
        };
        let note_at = |np: &NowPlayingFaces, art_in| {
            let mut out = Vec::new();
            draw_art(np, art_in, &theme, &mut out);
            out.iter().find_map(|d| match *d {
                Draw::Tex { tex, alpha, x, .. } if tex == note => Some((alpha, x)),
                _ => None,
            })
        };
        let middle = QUICK_EDGE + (ART_BOX - 96) as f32 / 2.0;
        assert_eq!(note_at(&np(None), 0.0), Some((1.0, middle)), "centred");
        let shown = np(Some((picture, 240, 240)));
        assert_eq!(note_at(&shown, 0.25).map(|n| n.0), Some(0.75), "going");
        assert_eq!(note_at(&shown, 1.0).map(|n| n.0), Some(0.0), "gone");
    }
}
