//! The menus START opens: Now Playing's, for the song playing (Favorites, the sleep timer,
//! repeat, go to its album or artist, Song Info); Up Next's, to save the queue as a playlist
//! or clear it; and a list's, for what no key does on the song or book picked: go to its album
//! or artist and Song Info, or start the book over and mark it finished. START or B closes them.

use super::{list, settings, Body, Screen};
use crate::browse::rows::{self, Row};
use crate::browse::{Browser, Context, Key, Outcome, View};
use crate::library::TrackId;
use crate::{text, Music};

/// START: opens the menu for what is shown, or closes the one open.
pub(crate) fn toggle(b: &mut Browser, cx: &Context) {
    let view = b.view();
    match view {
        View::NowPlaying if cx.queue.tracks.get(cx.queue.at).is_some() => {
            b.push(View::PlayingMenu, 0)
        }
        View::Queue if !cx.queue.tracks.is_empty() => b.push(View::QueueMenu, 0),
        View::PlayingMenu
        | View::QueueMenu
        | View::SongMenu(_)
        | View::BookMenu(_)
        | View::SongInfo(_) => b.pop(),
        _ => {
            // A list's songs and books have menus; its other rows do all with their keys.
            let rows = rows::rows(cx, view);
            match rows.get(b.at()) {
                Some(&Row::Track(t)) => b.push(View::SongMenu(t), 0),
                Some(&Row::Book(book)) => b.push(View::BookMenu(book), 0),
                _ => {}
            }
        }
    }
}

/// The menus' rows. Each acts on the menu's song, or on what the list's menu was opened for.
pub(crate) fn press(b: &mut Browser, cx: &Context, row: Row, key: Key) -> Option<Outcome> {
    match row {
        Row::Setting(s) => return settings::press(s, key),
        // Song Info only tells: no row to move to or pick.
        Row::Info(_) => return Some(Outcome::None),
        // Shuffle turns on or off as a setting does, beside Repeat.
        Row::ShuffleQueue => {
            return matches!(key, Key::A | Key::Left | Key::Right).then_some(Outcome::ShuffleQueue);
        }
        // A book's speed steps as a setting does.
        Row::Speed => {
            let by = match key {
                Key::Left => -1,
                Key::Right => 1,
                Key::A => 0,
                _ => return None,
            };
            return Some(Outcome::Speed(by));
        }
        _ => {}
    }
    if key != Key::A {
        return None;
    }
    // The queue's menu acts and closes; the queue, cleared, shows what plays.
    if b.view() == View::QueueMenu {
        b.pop();
        return Some(match row {
            Row::SaveQueue => Outcome::SaveQueue,
            _ => {
                b.set_at(0);
                Outcome::Clear
            }
        });
    }
    let song = rows::subject(cx.queue, b.view());
    // The book playing's menu: its chapters and bookmarks open over Now Playing, as the queue
    // does; going back and finishing it act from there.
    if let (View::PlayingMenu, Some(book)) = (b.view(), cx.book) {
        let outcome = match row {
            Row::Chapters | Row::Bookmarks => {
                b.pop();
                match row {
                    Row::Chapters => super::book::open_book_chapters(b, cx, book),
                    _ => b.push(View::Bookmarks(book), 0),
                }
                return Some(Outcome::None);
            }
            Row::Finished => Outcome::Finish(book),
            Row::Back => Outcome::Back,
            _ => return Some(Outcome::None),
        };
        b.pop();
        return Some(outcome);
    }
    // A book's menu: its chapters, start it over, or mark it finished or not.
    if let View::BookMenu(book) = b.view() {
        b.pop();
        return Some(match row {
            Row::Chapters => {
                super::book::open_book_chapters(b, cx, book);
                Outcome::None
            }
            Row::Finished => Outcome::Finish(book),
            _ => {
                b.push(View::NowPlaying, 0);
                Outcome::PlayBook { book, over: true }
            }
        });
    }
    Some(match (row, song) {
        (Row::GoToAlbum | Row::GoToArtist, Some(t)) => {
            go_to(b, cx, row, t);
            Outcome::None
        }
        // The menu stays: its row now offers the other way.
        (Row::SongInfo, Some(t)) => {
            b.push(View::SongInfo(t), 0);
            Outcome::None
        }
        _ => Outcome::None,
    })
}

/// The song's album or artist, on the song. The menu closes, so B returns to
/// where it was opened.
fn go_to(b: &mut Browser, cx: &Context, row: Row, t: TrackId) {
    let album = cx.lib.tracks[t].album;
    let (view, on) = match row {
        Row::GoToAlbum => (
            View::Album(album),
            cx.lib.albums[album].tracks.iter().position(|&x| x == t),
        ),
        _ => {
            let artist = cx.lib.albums[album].artist;
            let albums = &cx.lib.artists[artist].albums;
            (
                View::Artist(artist),
                albums.iter().position(|&a| a == album),
            )
        }
    };
    b.pop();
    b.push(view, rows::fixed(view).len() + on.unwrap_or(0));
}

pub(crate) fn screen(music: &Music) -> Screen {
    match music.browser.view() {
        View::PlayingMenu if music.player.current().is_none() => {
            Screen::message(text::NOW_PLAYING, text::NOTHING_PLAYING)
        }
        // Facts to read, not rows to pick: no count, no highlight.
        View::SongInfo(_) => {
            let mut screen = list::rows_screen(music);
            if let Body::List(list) = &mut screen.body {
                list.readonly = true;
            }
            Screen {
                corner: String::new(),
                ..screen
            }
        }
        _ => list::rows_screen(music),
    }
}
