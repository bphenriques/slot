//! An audiobook playing, as players have it: its chapter large, then its author and the book;
//! the bar and the time are the chapter's, and under the picture, the speed, how far through
//! the whole book it is and the time it has left at that speed, with ticks on the bar where
//! bookmarks are. Left and Right jump back and on, L1 and R1 change chapter, X steps the speed
//! and Y marks the moment, or held lists the bookmarks. SELECT lists the chapters; START's menu
//! has them, the bookmarks, the speed and the sleep timer.

use super::{list, now_playing, Body, NowPlayingBody, Screen, Status};
use crate::books;
use crate::browse::rows::{self, Row};
use crate::browse::{Browser, Context, Key, Outcome, View};
use crate::library::{BookId, TrackId};
use crate::playback::player::Repeat;
use crate::{text, Music};
use slot_player::PlayState;

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

/// A book's chapters, all its parts', on the one it was left in or plays: from its Now Playing,
/// its list or its menu.
pub(crate) fn open_book_chapters(b: &mut Browser, cx: &Context, book: BookId) {
    let view = View::BookChapters(book);
    let at = rows::left_at(cx, &rows::rows(cx, view), book);
    b.push(view, at);
}

/// A chapter or bookmark picked plays, on Now Playing; X takes a bookmark out.
pub(crate) fn press_row(b: &mut Browser, row: Row, key: Key) -> Option<Outcome> {
    let outcome = match (row, key, b.view()) {
        (
            Row::BookChapter {
                book,
                part,
                chapter,
            },
            Key::A,
            _,
        ) => Outcome::BookChapter {
            book,
            part,
            chapter,
        },
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

/// What a book's Now Playing shows of its time: where in the part it plays, the chapter's
/// start and end there, how fast; and the whole book's time left and how far through it is, when
/// its parts' lengths are known, else the part's.
struct Timing {
    pos: f64,
    start: f64,
    end: f64,
    speed: f32,
    left: f64,
    done: Option<f64>,
    secs: f64,
}

fn timing(music: &Music, book: BookId) -> Timing {
    let (lib, player) = (&music.library, &music.player);
    let (pos, len) = player.time();
    let (start, end) = player
        .chapter()
        .map_or((0.0, len), |c| (c.chapter.start, c.end));
    let b = &lib.books[book];
    let (_, part) = music.book().unwrap_or_default();
    let (left, done) = if b.secs > 0.0 {
        let into = books::into_book(lib, book, part, pos);
        (b.secs - into, Some((into / b.secs).clamp(0.0, 1.0)))
    } else {
        (len - pos, None)
    };
    let speed = player.speed();
    Timing {
        pos,
        start,
        end,
        speed,
        left,
        done,
        secs: b.secs,
    }
}

/// Seconds until a book playing shows it moved on, as `screen` shows it: the chapter's time
/// and bar, the book's share done and time left, and the chapter's end.
pub(crate) fn steps_in(music: &Music, book: BookId) -> Option<f64> {
    if music.player.state() != PlayState::Playing {
        return None;
    }
    let t = timing(music, book);
    let rate = f64::from(t.speed);
    let (into, chapter) = (t.pos - t.start, t.end - t.start);
    let bar = (chapter > 0.0)
        .then(|| super::whole_in(into / chapter * 1000.0 + 0.5, rate * 1000.0 / chapter));
    let done = t
        .done
        .map(|d| super::whole_in(d * 100.0 + 0.5, rate * 100.0 / t.secs));
    let steps = [
        super::whole_in(into, rate),
        bar.flatten(),
        done.flatten(),
        super::whole_in(t.left.max(0.0) / rate, -1.0),
        Some((t.end - t.pos).max(0.0) / rate),
    ];
    steps.into_iter().flatten().reduce(f64::min)
}

/// Now Playing for `book`, of which `track` plays.
pub(crate) fn screen(music: &Music, book: BookId, track: TrackId) -> Screen {
    let (lib, player) = (&music.library, &music.player);
    let Timing {
        pos,
        start,
        end,
        speed,
        left,
        done,
        ..
    } = timing(music, book);
    let (title, (n, of)) = match player.chapter() {
        // A chapter with no title of its own is named as its part is.
        Some(c) => (
            if c.chapter.title.is_empty() {
                lib.tracks[track].title.clone()
            } else {
                c.chapter.title.clone()
            },
            (c.index + 1, player.chapters()),
        ),
        None => (lib.tracks[track].title.clone(), player.place()),
    };
    let (_, part) = music.book().unwrap_or_default();
    let b = &lib.books[book];
    // The bookmarks in the chapter playing, along its bar.
    let marks = music.books.shelf.bookmarks(lib, book).iter();
    let marks = marks
        .filter(|m| m.part == part && (start..end).contains(&m.pos))
        .map(|m| ((m.pos - start) / (end - start)) as f32)
        .collect();
    let playing = player.state() == PlayState::Playing;
    let body = NowPlayingBody {
        title,
        by: lib.artists[lib.albums[b.album].artist].name.clone(),
        from: lib.albums[b.album].name.clone(),
        status: Status {
            playing,
            repeat: Repeat::Off,
            shuffled: false,
            favorite: false,
            sleeping: music.transport.sleep.left(music.now.at).is_some(),
        },
        progress: super::fraction(pos - start, end - start),
        marks,
        place: text::of(n, of),
        time: text::elapsed(pos - start, end - start),
        next: Some(text::book_left(
            speed,
            done,
            left.max(0.0) / f64::from(speed),
        )),
        color: music.settings.color,
        book: true,
    };
    Screen::new(
        text::NOW_PLAYING,
        super::legend(&music.browser, &music.context()),
        Body::NowPlaying(body),
    )
}

/// A book's chapters, or its bookmarks.
pub(crate) fn list(music: &Music) -> Screen {
    if let View::Bookmarks(book) = music.browser.view() {
        if music.books.shelf.bookmarks(&music.library, book).is_empty() {
            return Screen::message(text::BOOKMARKS, text::NO_BOOKMARKS);
        }
    }
    list::rows_screen(music)
}
