//! A list of rows, as every screen but Now Playing is: the library's lists here, and the main
//! menu, settings and queue on top of it.

use crate::library::sort::initial;

use super::{Body, Legend, ListBody, Screen};
use crate::books;
use crate::browse::rows::{self, Row};
use crate::browse::{per_page, Browser, Context, Key, Outcome, Shuffle, View};
use crate::text;
use crate::Music;

/// A held puts the row's songs next.
pub(crate) fn held(cx: &Context, view: View, row: Row, key: Key) -> Outcome {
    match (key, rows::tracks(cx, view, row)) {
        (Key::A, Some(tracks)) => Outcome::PlayNext(tracks),
        _ => Outcome::None,
    }
}

/// Up and Down go round; Left and Right turn a page; the shoulders jump a letter in a list by
/// name, and elsewhere turn a page too, but on a menu, whose Left and Right change a setting, do
/// nothing: SELECT with one skips the song from anywhere. A opens or plays; X adds the row's
/// songs to the end of the queue; Y puts a song in Favorites or takes it out.
pub(crate) fn press(b: &mut Browser, cx: &Context, rows: &[Row], at: usize, key: Key) -> Outcome {
    let (n, view) = (rows.len(), b.view());
    let key = match key {
        Key::L1 | Key::R1 if view.is_menu() => return Outcome::None,
        Key::L1 if !view.by_name() => Key::Left,
        Key::R1 if !view.by_name() => Key::Right,
        key => key,
    };
    let to = match key {
        Key::Up => (at + n - 1) % n,
        Key::Down => (at + 1) % n,
        Key::Left => at.saturating_sub(per_page(view)),
        Key::Right => (at + per_page(view)).min(n - 1),
        Key::L1 => rows::letter(cx.lib, rows, at, false),
        Key::R1 => rows::letter(cx.lib, rows, at, true),
        Key::X => {
            // A book's chapters, from the list.
            if let Row::Book(book) = rows[at] {
                super::book::open_book_chapters(b, cx, book);
                return Outcome::None;
            }
            return rows::tracks(cx, view, rows[at]).map_or(Outcome::None, Outcome::Enqueue);
        }
        // Y puts a song in Favorites, or takes it out.
        Key::Y => {
            return match rows[at] {
                Row::Track(t) => Outcome::Favorite(t),
                // A book's bookmarks, from the list.
                Row::Book(book) => {
                    b.push(View::Bookmarks(book), 0);
                    Outcome::None
                }
                _ => Outcome::None,
            };
        }
        Key::A => return open(b, cx, rows[at], at),
        _ => return Outcome::None,
    };
    b.set_at(to);
    // A letter jump names the letter it landed on.
    match key {
        Key::L1 | Key::R1 => rows::name(cx.lib, rows[to])
            .map_or(Outcome::None, |name| Outcome::Letter(initial(name))),
        _ => Outcome::None,
    }
}

/// A on a row: open its screen, or play.
fn open(b: &mut Browser, cx: &Context, row: Row, at: usize) -> Outcome {
    let view = b.view();
    let fixed = rows::fixed(view).len();
    let (to, outcome) = match row {
        // With nothing to resume, it shuffles every song; with no songs, it waits for some.
        Row::NowPlaying if rows::idle(cx.queue) && cx.lib.tracks_by_title.is_empty() => {
            return Outcome::None
        }
        Row::NowPlaying if rows::idle(cx.queue) => {
            let shuffle = Outcome::Play {
                tracks: cx.lib.music_tracks(),
                start: 0,
                shuffle: Shuffle::Tracks,
            };
            (View::NowPlaying, shuffle)
        }
        // Resume plays on, as it says; even before the saved queue is back, once it is.
        Row::NowPlaying if !cx.queue.playing => (View::NowPlaying, Outcome::Toggle),
        Row::NowPlaying => (View::NowPlaying, Outcome::None),
        Row::Open(view) => (view, Outcome::None),
        Row::Genre(g) => (View::Genre(g), Outcome::None),
        Row::Decade(d) => (View::Decade(d), Outcome::None),
        Row::Playlist(p) => (View::Playlist(p), Outcome::None),
        Row::Author(a) => (View::Author(a), Outcome::None),
        // The latest book plays on, as Audiobooks held does from the Music menu.
        Row::Continue => match books::latest(cx.lib, cx.shelf) {
            Some(book) => (View::NowPlaying, Outcome::PlayBook { book, over: false }),
            None => return Outcome::None,
        },
        // A book plays on where it was left.
        Row::Book(book) => (View::NowPlaying, Outcome::PlayBook { book, over: false }),
        Row::Artist(a) => (View::Artist(a), Outcome::None),
        Row::Album(a) => (View::Album(a), Outcome::None),
        Row::Rescan => return Outcome::Rescan(false),
        Row::Setting(s) => return Outcome::Change(s, 0),
        // The START menus answer these themselves.
        Row::StartOver
        | Row::Finished
        | Row::Chapters
        | Row::Bookmarks
        | Row::BookChapter { .. }
        | Row::Bookmark(_)
        | Row::Speed
        | Row::Back
        | Row::Favorite
        | Row::GoToAlbum
        | Row::GoToArtist
        | Row::SongInfo
        | Row::ShuffleQueue
        | Row::SaveQueue
        | Row::ClearQueue
        | Row::Info(_) => return Outcome::None,
        // A pick in the queue returns to Now Playing, which opened it.
        Row::Queued(i) if b.under_now_playing() => {
            b.pop();
            return Outcome::Jump(i);
        }
        Row::Queued(i) => (View::NowPlaying, Outcome::Jump(i)),
        Row::PlayAll | Row::Shuffle => {
            let Some(tracks) = rows::tracks(cx, view, row) else {
                return Outcome::None;
            };
            // Shuffle deals what the list shows: albums or artists whole, or tracks.
            let shuffle = match (row, rows::groups(cx.lib, view)) {
                (Row::PlayAll, _) => Shuffle::Off,
                (_, Some(sizes)) => Shuffle::Groups(sizes),
                (_, None) => Shuffle::Tracks,
            };
            let play = Outcome::Play {
                tracks,
                start: 0,
                shuffle,
            };
            (View::NowPlaying, play)
        }
        // From this song, the list it is in: the songs Play All plays.
        Row::Track(_) => {
            let Some(tracks) = rows::tracks(cx, view, Row::PlayAll) else {
                return Outcome::None;
            };
            let play = Outcome::Play {
                tracks,
                start: at - fixed,
                shuffle: Shuffle::Off,
            };
            (View::NowPlaying, play)
        }
    };
    b.push(to, 0);
    outcome
}

/// A library or audiobooks list, once the library is read, or where to put what it would list.
pub(crate) fn screen(music: &Music) -> Screen {
    let (lib, view) = (&music.library, music.browser.view());
    let books = matches!(
        view,
        View::Books | View::BookList(_) | View::Authors | View::Author(_)
    );
    if !music.scan.ready {
        let (done, of) = music.reading();
        Screen::message(text::MUSIC, &text::reading(done, of))
    } else if books && lib.books.is_empty() {
        Screen::message(text::BOOKS, text::NO_BOOKS)
    } else if !books && lib.tracks_by_title.is_empty() {
        Screen::message(text::title(view).unwrap_or(text::MUSIC), text::NO_MUSIC)
    } else {
        rows_screen(music)
    }
}

/// The page the cursor is on, with the hints `legend` gives for its highlighted row. Lists
/// count their items in the corner; menus do not.
pub(crate) fn rows_screen(music: &Music) -> Screen {
    let view = music.browser.view();
    let cx = music.context();
    let page = music.browser.page(&cx);
    let legend = super::legend(&music.browser, &cx);
    let count = match (view.is_menu(), page.item) {
        (true, _) => String::new(),
        (false, Some(i)) => text::of(i, page.items),
        (false, None) => page.items.to_string(),
    };
    // A menu on one page is centred on its rows; over pages, on a full page, so it holds still.
    let per_page = if view.is_menu() && page.pages == 1 {
        page.rows.len()
    } else {
        page.per_page
    };
    let list = ListBody {
        rows: page.rows,
        values: page.values,
        selected: page.selected,
        adjustable: matches!(legend, Legend::Settings | Legend::Menu),
        menu: view.is_menu(),
        per_page,
        rules: page.rules,
        progress: None,
        lifted: false,
        readonly: false,
        thumbs: page.thumbs,
    };
    Screen {
        corner: count,
        ..Screen::new(page.title, legend, Body::List(list))
    }
}
