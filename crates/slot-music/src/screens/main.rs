//! The music main menu: Now Playing, the library's lists and Settings. It moves as a list does,
//! and reports the card being checked behind the library shown, then the album pictures being
//! made. A held on a list shuffles it all, as its Shuffle row does; on Audiobooks, it plays on
//! the book listened to last.

use super::{list, Body, Legend, MainHold, Screen};
use crate::browse::rows::{self, Row};
use crate::browse::{Browser, Context, Key, Outcome, Shuffle, View};
use crate::library::BookId;
use crate::text;
use crate::Music;

/// What A held does on `row`, if anything.
pub(crate) fn hold(cx: &Context, row: Row) -> Option<MainHold> {
    match row {
        Row::Artists | Row::Albums | Row::Tracks => Some(MainHold::Shuffle),
        Row::Books => latest(cx).map(|_| MainHold::Resume),
        _ => None,
    }
}

/// The book listened to last, unless it was finished.
fn latest(cx: &Context) -> Option<BookId> {
    let place = |b| cx.shelf.place(cx.lib, b);
    (0..cx.lib.books.len())
        .map(BookId)
        .filter(|&b| place(b).when > 0 && !place(b).finished)
        .max_by_key(|&b| place(b).when)
}

/// Held on `row`: A shuffles its list, or plays on the latest book.
pub(crate) fn held(b: &mut Browser, cx: &Context, row: Row, key: Key) -> Outcome {
    if key != Key::A {
        return Outcome::None;
    }
    let view = match row {
        Row::Artists => View::Artists,
        Row::Albums => View::Albums,
        Row::Tracks => View::Tracks,
        Row::Books => {
            let Some(book) = latest(cx) else {
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
    let playing = !rows::idle(music.player.queue());
    let cx = music.context();
    let mut screen = list::rows_screen(music, |row| Legend::Main {
        playing,
        hold: row.and_then(|row| hold(&cx, row)),
    });
    if !screen.corner.is_empty() {
        return screen;
    }
    if music.scanning() {
        let (done, of) = music.reading();
        screen.corner = text::scanning(music.scan.ready, done, of);
    } else if let (Body::List(list), Some((done, of))) =
        (&mut screen.body, music.pictures.covers.progress())
    {
        list.progress = Some(done as f32 / of as f32);
        screen.corner = text::optimizing(done, of);
    }
    screen
}
