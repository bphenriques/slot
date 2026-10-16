//! Now Playing's faces: the track lit and its artist and album smaller, all scrolling when
//! long; the album picture and its colour; the state symbols; the queue place, time and the
//! next track.

use slot_gfx::Compositor;
use slot_input::Millis;
use slot_music::{NowPlayingBody, Repeat, Status};
use slot_ui::{
    music_text_width, music_tint, music_title_w, Canvas, LineInk, MusicFace, MusicTint,
    NowPlayingFaces,
};

use super::{put, Glyphs, MusicText, Spec};
use crate::app::App;

/// An album picture on the GPU, and its colour for Dynamic Color.
#[derive(Copy, Clone)]
pub(super) struct Picture {
    album: usize,
    face: MusicFace,
    pub(super) tint: [u8; 3],
}

pub(super) fn faces(
    text: &mut MusicText,
    app: &App,
    compositor: &mut Compositor,
    canvas: Canvas,
    body: NowPlayingBody,
    glyphs: Glyphs,
    now: Millis,
) -> NowPlayingFaces {
    let scroll = Some(text.scroll.ms((None, body.title.clone()), now));
    let width = music_text_width(true, canvas);
    let lines = [
        (0, body.title, LineInk::Lit),
        (1, body.by, LineInk::Second),
        (2, body.from, LineInk::Second),
    ]
    .map(|(i, line, ink)| {
        text.lines.sync(
            compositor,
            text.theme,
            i,
            Spec::Line(line, ink),
            width,
            scroll,
        )
    });
    let half = music_title_w(false, canvas);
    let place = text.lines.sync(
        compositor,
        text.theme,
        3,
        Spec::small(body.place),
        half,
        None,
    );
    let time = text.lines.sync(
        compositor,
        text.theme,
        4,
        Spec::small(body.time),
        half,
        None,
    );
    let next = body.next.and_then(|next| {
        let wide = music_text_width(false, canvas);
        text.lines
            .sync(compositor, text.theme, 5, Spec::small(next), wide, scroll)
    });
    let art = art(text, app, compositor, now);
    let tint = art.filter(|_| body.color).map(|p| MusicTint {
        now: p.tint,
        was: text.art_was.map(|w| w.tint),
    });
    NowPlayingFaces {
        lines,
        art: art.map(|p| p.face),
        art_was: text.art_was.map(|p| p.face),
        status: shown(body.status, glyphs),
        progress: body.progress,
        marks: body.marks,
        place,
        time,
        next,
        tint,
    }
}

/// The playing album's picture, uploaded when the album changes, fading in over the last.
fn art(
    text: &mut MusicText,
    app: &App,
    compositor: &mut Compositor,
    now: Millis,
) -> Option<Picture> {
    let a = app.music_art()?;
    match text.art {
        Some(p) if p.album == a.album => Some(p),
        _ => {
            let mut tex = text.art_was.map(|p| p.face.0);
            let id = put(compositor, &mut tex, a.w, a.h, &a.rgba);
            let picture = Picture {
                album: a.album,
                face: (id, a.w, a.h),
                tint: music_tint(&a.rgba),
            };
            text.art_was = text.art.replace(picture);
            text.motion.new_art(now);
            Some(picture)
        }
    }
}

/// The state symbols: play or pause, then the sleep timer's moon, the heart, shuffle and repeat
/// when on.
fn shown(s: Status, glyphs: Glyphs) -> Vec<MusicFace> {
    let [play, pause, repeat, repeat_one, shuffle] = glyphs.status;
    let mut out = vec![if s.playing { play } else { pause }];
    if s.sleeping {
        out.push(glyphs.moon);
    }
    if s.favorite {
        out.push(glyphs.heart);
    }
    if s.shuffled {
        out.push(shuffle);
    }
    match s.repeat {
        Repeat::All => out.push(repeat),
        Repeat::One => out.push(repeat_one),
        Repeat::Off => {}
    }
    out
}
