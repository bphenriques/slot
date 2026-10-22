//! Now Playing's faces: the track lit and its artist and album smaller, all scrolling when
//! long; the album picture and its colour; the state symbols; the queue place, time and the
//! next track.

use super::Textures;
use slot_input::Millis;
use slot_music::{NowPlayingBody, Repeat, Status};
use slot_ui::music::{
    music_half_w, music_text_width, music_tint, LineInk, MusicFace, MusicTint, NowPlayingFaces,
};
use slot_ui::Canvas;

use super::{put, Glyphs, MusicText, Spec};
use crate::app::App;

/// An album picture on the GPU, and its colour for Dynamic Color.
#[derive(Copy, Clone)]
pub(super) struct Picture {
    album: slot_music::AlbumId,
    face: MusicFace,
    pub(super) tint: [u8; 3],
}

pub(super) fn faces(
    text: &mut MusicText,
    app: &App,
    compositor: &mut dyn Textures,
    canvas: Canvas,
    body: NowPlayingBody,
    glyphs: Glyphs,
    now: Millis,
) -> NowPlayingFaces {
    let bars = body.bar_heights();
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
    let half = music_half_w(canvas);
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
    // Up Next scrolls on a clock of its own: a new next track, as a shuffle deals one, starts
    // from its beginning rather than wherever the title has got to.
    let next = body.next.and_then(|next| {
        let wide = music_text_width(false, canvas);
        let scroll = Some(text.next_scroll.ms((None, next.clone()), now));
        text.lines
            .sync(compositor, text.theme, 5, Spec::small(next), wide, scroll)
    });
    // A book's speed, when not as recorded: a word at the end of the state symbols.
    let status = shown(body.status, glyphs);
    let speed = body.speed.and_then(|speed| {
        text.lines
            .sync(compositor, text.theme, 6, Spec::small(speed), half, None)
    });
    let art = art(text, app, compositor, now);
    let tint = art.map(|p| MusicTint {
        now: p.tint,
        was: text.art_was.map(|w| w.tint),
    });
    NowPlayingFaces {
        lines,
        art: art.map(|p| p.face),
        art_was: text.art_was.map(|p| p.face),
        no_art: Some(glyphs.no_art[usize::from(body.book)]),
        status,
        speed,
        bars,
        progress: body.progress,
        marks: body.marks,
        place,
        time,
        next,
        tint,
    }
}

/// The playing album's picture, uploaded when the album changes, fading in over the last; or,
/// the first on a screen still coming in, there at once, as it comes in with the screen.
fn art(
    text: &mut MusicText,
    app: &App,
    compositor: &mut dyn Textures,
    now: Millis,
) -> Option<Picture> {
    // None, the last is let go, so the next does not fade in over it.
    let Some(a) = app.music_art() else {
        let gone = [text.art.take(), text.art_was.take()];
        text.art_free
            .extend(gone.into_iter().flatten().map(|p| p.face.0));
        return None;
    };
    match text.art {
        Some(p) if p.album == a.album => Some(p),
        _ => {
            let mut tex = text
                .art_was
                .map(|p| p.face.0)
                .or_else(|| text.art_free.pop());
            let id = put(compositor, &mut tex, a.w, a.h, &a.rgba);
            let picture = Picture {
                album: a.album,
                face: (id, a.w, a.h),
                tint: music_tint(&a.rgba),
            };
            text.art_was = text.art.replace(picture);
            let fade = text.art_was.is_some() || !text.motion.entering(now);
            text.motion.new_art(now, fade);
            Some(picture)
        }
    }
}

/// The state symbols: play or pause, then the heart, shuffle and repeat when on. The sleep
/// timer says itself in the corner.
fn shown(s: Status, glyphs: Glyphs) -> Vec<MusicFace> {
    let [play, pause, repeat, repeat_one, shuffle] = glyphs.status;
    let mut out = vec![if s.playing { play } else { pause }];
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
