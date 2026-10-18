//! Now Playing: the track, its album picture, the state symbols and the bar. A pauses, X
//! shows the lyrics, Down shuffles, Y puts the song in Favorites and Up steps the repeat round, as START's menu does;
//! SELECT opens the queue. The back buttons carry the transport: L1 and R1 change track, as
//! Left and Right do, and L2 and R2 seek. An audiobook has its own, in `book`.

use super::{Body, NowPlayingBody, Screen, Status};
use crate::browse::{Browser, Context, Key, Outcome, View};
use crate::settings::Setting;
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

/// X opens the lyrics; Up and Down, side by side, step the repeat round, as START's menu row
/// does, and shuffle the queue or put it back in order.
pub(crate) fn press(b: &mut Browser, cx: &Context, key: Key) -> Outcome {
    match key {
        Key::A => Outcome::Toggle,
        Key::X if cx.lyrics.lines > 0 => {
            b.push(View::Lyrics, 0);
            Outcome::None
        }
        Key::Up => Outcome::Change(Setting::Repeat, 0),
        Key::Down => Outcome::ShuffleQueue,
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

pub(crate) fn screen(music: &Music) -> Screen {
    if music.gone {
        return Screen::message(text::NOW_PLAYING, text::NO_PLAYER);
    }
    let Some(track) = music.player.current() else {
        // The saved queue comes back once the library is read; until then, how far it is.
        let line = match music.player.queue().waiting {
            true => {
                let (done, of) = music.reading();
                text::reading(done, of)
            }
            false => text::NOTHING_PLAYING.into(),
        };
        return Screen::message(text::NOW_PLAYING, &line);
    };
    if let Some((book, _)) = music.book() {
        return super::book::screen(music, book, track);
    }
    let (lib, player) = (&music.library, &music.player);
    let (at, len) = player.time();
    let (n, of) = player.place();
    // The bar moves on between the player's reports.
    let bar = player.time().0;
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
            sleeping: music.transport.sleep.left(music.now.at).is_some(),
        },
        progress: (len > 0.0).then(|| (bar / len).clamp(0.0, 1.0) as f32),
        marks: Vec::new(),
        place: text::of(n, of),
        time: text::elapsed(at, len),
        next: player
            .up_next()
            .filter(|_| music.settings.up_next)
            .map(|t| text::up_next(&lib.tracks[t].title, &lib.tracks[t].artist)),
        color: music.settings.color,
    };
    Screen::new(
        text::NOW_PLAYING,
        super::legend(&music.browser, &music.context()),
        Body::NowPlaying(body),
    )
}
