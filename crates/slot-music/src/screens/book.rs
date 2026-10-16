//! An audiobook playing, as players have it: its chapter large, then its author and the book;
//! the bar and the time are the chapter's, and under the picture, the speed, how far through
//! the whole book it is and the time it has left at that speed, with ticks on the bar where
//! bookmarks are. Left and Right jump back and on, L1 and R1 change chapter, X steps the speed
//! and Y marks the moment, or held lists the bookmarks. SELECT lists the chapters; START's menu
//! has them, the bookmarks, the speed and the sleep timer.

use super::{list, now_playing, Body, Legend, NowPlayingBody, Screen, Status};
use crate::books;
use crate::browse::{Browser, Context, Key, Outcome, View};
use crate::keys::Mode;
use crate::muse::MuseState;
use crate::player::Repeat;
use crate::rows::Row;
use crate::{text, Music};

/// As music's: Left and Right step tapped and seek held. Y waits to see whether it is held.
pub(crate) fn mode(key: Key) -> Mode {
    match key {
        Key::Y => Mode::Hold,
        key => now_playing::mode(key),
    }
}

/// Left and Right held seek; Y held lists the bookmarks, as it adds one tapped.
pub(crate) fn held(b: &mut Browser, cx: &Context, key: Key) -> Outcome {
    if let (Key::Y, Some(book)) = (key, cx.book) {
        b.push(View::Bookmarks(book), 0);
        return Outcome::None;
    }
    now_playing::held(key)
}

/// Left and Right jump as far as Jump Length says, as do L2 and R2.
pub(crate) fn press(cx: &Context, key: Key) -> Outcome {
    let jump = cx.settings.jump as i32;
    match key {
        Key::A => Outcome::Toggle,
        Key::X => Outcome::Speed(0),
        Key::Y => Outcome::Bookmark,
        Key::Left | Key::L2 => Outcome::Seek(-jump),
        Key::Right | Key::R2 => Outcome::Seek(jump),
        Key::L1 => Outcome::Skip(-1),
        Key::R1 => Outcome::Skip(1),
        _ => Outcome::None,
    }
}

/// The chapters, on the one playing.
pub(crate) fn open_chapters(b: &mut Browser, cx: &Context) {
    b.push(View::Chapters, cx.queue.chapter.unwrap_or(cx.queue.at));
}

/// A chapter or bookmark picked plays, on Now Playing; X takes a bookmark out.
pub(crate) fn press_row(b: &mut Browser, row: Row, key: Key) -> Option<Outcome> {
    let outcome = match (row, key, b.view()) {
        (Row::Chapter(i), Key::A, _) => Outcome::Chapter(i),
        (Row::Bookmark(mark), Key::A, View::Bookmarks(book)) => Outcome::GoBookmark { book, mark },
        (Row::Bookmark(mark), Key::X, View::Bookmarks(book)) => {
            return Some(Outcome::RemoveBookmark { book, mark })
        }
        _ => return None,
    };
    // Opened over Now Playing it goes back there; from the books, on to it.
    b.pop();
    if b.view() != View::NowPlaying {
        b.push(View::NowPlaying, 0);
    }
    Some(outcome)
}

/// Now Playing for `book`, of which `track` plays.
pub(crate) fn screen(music: &Music, book: usize, track: usize) -> Screen {
    let (lib, player) = (&music.library, &music.player);
    let (pos, len) = (player.smooth_pos(), player.time().1);
    let (title, start, end, (n, of)) = match player.chapter() {
        Some((i, title, start, end)) => (title.into(), start, end, (i + 1, player.chapters())),
        None => (lib.tracks[track].title.clone(), 0.0, len, player.place()),
    };
    let speed = player.speed();
    let b = &lib.books[book];
    let (_, part) = music.book().unwrap_or_default();
    // The whole book's time left and how far through it is, when its parts' lengths are known.
    let (left, done) = match b.secs > 0.0 {
        true => {
            let into = books::into_book(lib, book, part, pos);
            (b.secs - into, Some((into / b.secs).clamp(0.0, 1.0)))
        }
        false => (len - pos, None),
    };
    // The bookmarks in the chapter playing, along its bar.
    let marks = music.shelf.bookmarks(lib, book).iter();
    let marks = marks
        .filter(|m| m.part == part && (start..end).contains(&m.pos))
        .map(|m| ((m.pos - start) / (end - start)) as f32)
        .collect();
    let playing = player.state() == MuseState::Playing;
    let body = NowPlayingBody {
        title,
        by: lib.artists[lib.albums[b.album].artist].name.clone(),
        from: lib.albums[b.album].name.clone(),
        status: Status {
            playing,
            repeat: Repeat::Off,
            shuffled: false,
            favorite: false,
            sleeping: music.sleep.left().is_some(),
        },
        progress: (end > start).then(|| ((pos - start) / (end - start)).clamp(0.0, 1.0) as f32),
        marks,
        place: text::of(n, of),
        time: text::elapsed(pos - start, end - start),
        next: Some(text::book_left(
            speed,
            done,
            left.max(0.0) / f64::from(speed),
        )),
        color: music.settings.color,
    };
    Screen::new(
        text::NOW_PLAYING,
        Legend::Book {
            playing,
            jump: music.settings.jump,
        },
        Body::NowPlaying(body),
    )
}

/// The chapters of the book playing, or a book's bookmarks.
pub(crate) fn list(music: &Music) -> Screen {
    match music.browser.view() {
        View::Bookmarks(book) => match music.shelf.bookmarks(&music.library, book) {
            [] => Screen::message(text::BOOKMARKS, text::NO_BOOKMARKS),
            _ => list::rows_screen(music, |_| Legend::Bookmarks),
        },
        _ if music.book().is_none() => Screen::message(text::NOW_PLAYING, text::NOTHING_PLAYING),
        _ => list::rows_screen(music, |_| Legend::Chapters),
    }
}
