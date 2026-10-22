//! Now Playing: the track, its picture, the state symbols and the bar. A pauses, X shows the
//! lyrics, Y favorites, SELECT opens the queue; Left, Right, L1 and R1 change track, held they
//! seek; L2 and R2 seek. Up and Down do nothing, as on other players: repeat and shuffle are in
//! START's menu. A book's is in `book`.

use super::{Body, NowPlayingBody, Screen, Status};
use crate::browse::{Browser, Context, Key, Outcome, View};
use crate::{text, Music};
use slot_player::PlayState;

/// Seconds a seek moves.
pub(crate) const SEEK_S: i32 = 10;

/// Held, the back keys seek back, the on keys on.
pub(crate) fn held(key: Key) -> Outcome {
    match key {
        Key::Left | Key::L1 | Key::L2 => Outcome::Seek(-SEEK_S),
        Key::Right | Key::R1 | Key::R2 => Outcome::Seek(SEEK_S),
        _ => Outcome::None,
    }
}

/// X opens the lyrics.
pub(crate) fn press(b: &mut Browser, cx: &Context, key: Key) -> Outcome {
    match key {
        Key::A => Outcome::Toggle,
        Key::X if cx.lyrics.lines > 0 => {
            b.push(View::Lyrics, 0);
            Outcome::None
        }
        Key::Y => match cx.queue.tracks.get(cx.queue.at) {
            Some(&t) => Outcome::Favorite(t),
            None => Outcome::None,
        },
        Key::Left | Key::L1 => Outcome::Skip(-1),
        Key::Right | Key::R1 => Outcome::Skip(1),
        Key::L2 => Outcome::Seek(-SEEK_S),
        Key::R2 => Outcome::Seek(SEEK_S),
        _ => Outcome::None,
    }
}

/// The queue, at the track playing.
pub(crate) fn open_queue(b: &mut Browser, cx: &Context) {
    b.push(View::Queue, cx.queue.at);
}

/// Seconds until a song playing shows it moved on: its time's next second, its bar's next
/// step, as `screen` shows them.
pub(crate) fn steps_in(music: &Music) -> Option<f64> {
    let player = &music.player;
    if player.current().is_none() || player.state() != PlayState::Playing {
        return None;
    }
    let ((at, len), rate) = (player.time(), f64::from(player.speed()));
    let time = super::whole_in(at, rate);
    let bar = (len > 0.0)
        .then(|| super::whole_in(at / len * 1000.0 + 0.5, rate * 1000.0 / len))
        .flatten();
    [time, bar].into_iter().flatten().reduce(f64::min)
}

pub(crate) fn screen(music: &Music) -> Screen {
    if music.gone {
        return Screen::message(text::NOW_PLAYING, text::NO_PLAYER);
    }
    let Some(track) = music.player.current() else {
        // The saved queue comes back once the library is read; until then, how far it is.
        let line = if music.player.queue().waiting {
            let (done, of) = music.reading();
            text::reading(done, of)
        } else {
            text::NOTHING_PLAYING.into()
        };
        return Screen::message(text::NOW_PLAYING, &line);
    };
    if let Some((book, _)) = music.book() {
        return super::book::screen(music, book, track);
    }
    let (lib, player) = (&music.library, &music.player);
    let (at, len) = player.time();
    let (n, of) = player.place();
    let playing = player.state() == PlayState::Playing;
    let body = NowPlayingBody {
        title: lib.tracks[track].title.clone(),
        by: lib.tracks[track].artist.clone(),
        from: lib.album_of(track).name.clone(),
        status: Status {
            playing,
            repeat: player.repeat(),
            shuffled: player.shuffled(),
            favorite: lib.is_favorite(track),
        },
        progress: super::fraction(at, len),
        marks: Vec::new(),
        place: text::of(n, of),
        time: text::elapsed(at, len),
        next: player
            .up_next()
            .map(|t| text::up_next(&lib.tracks[t].title, &lib.tracks[t].artist)),
        book: false,
        speed: None,
        bars: music.bars(),
    };
    Screen::new(
        text::NOW_PLAYING,
        super::legend(&music.browser, &music.context()),
        Body::NowPlaying(body),
    )
}
