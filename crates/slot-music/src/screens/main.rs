//! The music main menu: Now Playing, the library's lists and Settings. It moves as a list does,
//! and reports the card being checked behind the library shown, then the album pictures being
//! made. A held on a list shuffles it all, as its Shuffle row does; on Audiobooks, it plays on
//! the book listened to last.

use super::{list, Body, MainHold, Screen};
use crate::books;
use crate::browse::rows::{self, Row};
use crate::browse::{Browser, Context, Key, Outcome, Shuffle, View};
use crate::text;
use crate::Music;

/// What A held does on `row`, if anything.
pub(crate) fn hold(cx: &Context, row: Row) -> Option<MainHold> {
    match row {
        Row::Open(View::Library)
        | Row::Open(View::Artists)
        | Row::Open(View::Albums)
        | Row::Open(View::Tracks) => Some(MainHold::Shuffle),
        Row::Open(View::Books) => books::latest(cx.lib, cx.shelf).map(|_| MainHold::Resume),
        _ => None,
    }
}

/// Held on `row`: A shuffles its list, or plays on the latest book.
pub(crate) fn held(b: &mut Browser, cx: &Context, row: Row, key: Key) -> Outcome {
    if key != Key::A {
        return Outcome::None;
    }
    let view = match row {
        // The library held shuffles every song, as Shuffle Songs does.
        Row::Open(View::Library) | Row::Open(View::Tracks) => View::Tracks,
        Row::Open(View::Artists) => View::Artists,
        Row::Open(View::Albums) => View::Albums,
        Row::Open(View::Books) => {
            let Some(book) = books::latest(cx.lib, cx.shelf) else {
                return Outcome::None;
            };
            b.push(View::NowPlaying, 0);
            return Outcome::PlayBook { book, over: false };
        }
        _ => return Outcome::None,
    };
    let Some(tracks) = rows::tracks(cx, view, Row::Shuffle) else {
        return Outcome::None;
    };
    let shuffle = rows::groups(cx.lib, view).map_or(Shuffle::Tracks, Shuffle::Groups);
    b.push(View::NowPlaying, 0);
    Outcome::Play {
        tracks,
        start: 0,
        shuffle,
    }
}

pub(crate) fn screen(music: &Music) -> Screen {
    let mut screen = list::rows_screen(music);
    if !screen.corner.is_empty() {
        return screen;
    }
    if music.scanning() {
        let (done, of) = music.reading();
        screen.corner = text::scanning(music.scan.ready, done, of);
        if let (Body::List(list), true) = (&mut screen.body, of > 0) {
            list.progress = Some(done as f32 / of as f32);
        }
    } else if music.library.tracks.is_empty() {
        screen.corner = text::NOTHING_YET.into();
    } else if let (Body::List(list), Some((done, of))) =
        (&mut screen.body, music.pictures.covers.progress())
    {
        list.progress = Some(done as f32 / of as f32);
        screen.corner = text::artwork(done, of);
    }
    screen
}
