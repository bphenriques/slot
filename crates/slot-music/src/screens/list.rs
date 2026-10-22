//! A list of rows, as every screen but Now Playing is: the library's lists here, and the main
//! menu, settings and queue on top of it.

use crate::library::sort::initial;
use crate::library::Library;

use super::{Body, Legend, ListBody, Screen};
use crate::books;
use crate::browse::rows::{self, Row};
use crate::browse::{page_of, page_rows, Browser, Context, Key, Outcome, Shuffle, View};
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
        Key::Left => turn(cx.lib, view, at, n, false),
        Key::Right => turn(cx.lib, view, at, n, true),
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
pub(crate) fn open(b: &mut Browser, cx: &Context, row: Row, at: usize) -> Outcome {
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
        // The queue opens on the song playing, as from Now Playing.
        Row::Open(View::Queue) => {
            b.push(View::Queue, cx.queue.at);
            return Outcome::None;
        }
        Row::Open(view) => (view, Outcome::None),
        Row::Genre(g) => (View::Genre(g), Outcome::None),
        Row::Folder(f) => (View::Folder(f), Outcome::None),
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
        // The START menus and the equalizer answer these themselves.
        Row::StartOver
        | Row::Finished
        | Row::Chapters
        | Row::Bookmarks
        | Row::BookChapter { .. }
        | Row::Bookmark(_)
        | Row::Speed
        | Row::Back
        | Row::GoToAlbum
        | Row::GoToArtist
        | Row::SongInfo
        | Row::ShuffleQueue
        | Row::SaveQueue
        | Row::ClearQueue
        | Row::Info(_)
        | Row::HelpLine(_)
        | Row::Preset(_)
        | Row::NewPreset
        | Row::Band(_) => return Outcome::None,
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
        // From this song, the list it is in: the songs Play All plays; in a folder, its own songs,
        // as a file browser plays a folder.
        Row::Track(t) => {
            let (tracks, start) = match view {
                View::Folder(f) => {
                    let own = cx.lib.folders[f].tracks.clone();
                    let start = own.iter().position(|&o| o == t).unwrap_or(0);
                    (own, start)
                }
                _ => match rows::tracks(cx, view, Row::PlayAll) {
                    Some(tracks) => (tracks, at - fixed),
                    None => return Outcome::None,
                },
            };
            let play = Outcome::Play {
                tracks,
                start,
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
        Screen::message(
            text::BOOKS,
            &text::no_books(&folder(&music.scan.dirs.books)),
        )
    } else if !books && lib.tracks_by_title.is_empty() {
        let line = text::no_music(&folder(&music.scan.dirs.music));
        Screen::message(text::title(view).unwrap_or(text::MUSIC), &line)
    } else if let Some((title, line)) = nothing_yet(music, view) {
        Screen::message(&title, line)
    } else {
        rows_screen(music)
    }
}

/// A list of songs with none in it yet, which says so rather than offer Play All of nothing:
/// its title, and what it says.
fn nothing_yet(music: &Music, view: View) -> Option<(String, &'static str)> {
    let lib = &music.library;
    match view {
        View::RecentlyPlayed if music.recent.tracks.is_empty() => {
            Some((text::RECENTLY_PLAYED.into(), text::NOTHING_PLAYED))
        }
        View::Playlist(0) if lib.playlists.first().is_some_and(|p| p.tracks.is_empty()) => {
            Some((text::FAVORITES.into(), text::NO_FAVORITES))
        }
        View::Playlist(p) if lib.playlists.get(p).is_some_and(|p| p.tracks.is_empty()) => {
            Some((lib.playlists[p].name.clone(), text::NO_SONGS))
        }
        _ => None,
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
        adjustable: matches!(legend, Legend::Settings | Legend::Menu | Legend::Tune),
        menu: view.is_menu(),
        per_page,
        rules: page.rules,
        progress: None,
        lifted: false,
        readonly: false,
        two_lines: page.two_lines,
        actions: crate::browse::rows::fixed(view).len(),
        paused: !cx.queue.playing,
        thumbs: page.thumbs,
        books: matches!(view, View::BookList(_) | View::Author(_) | View::Books),
        titled: view != View::Main,
    };
    Screen {
        corner: count,
        ..Screen::new(page.title, legend, Body::List(list))
    }
}

/// Where Left or Right (`on`) turns to from row `at` of `n`: the same place on the page before
/// or after, or the first or last row from the first or last page.
fn turn(lib: &Library, view: View, at: usize, n: usize, on: bool) -> usize {
    let page = page_of(lib, view, at);
    let place = at - page_rows(lib, view, page, n).start;
    let to = match (on, page) {
        (false, 0) => return 0,
        (false, page) => page_rows(lib, view, page - 1, n),
        (true, page) => page_rows(lib, view, page + 1, n),
    };
    if to.is_empty() {
        return n - 1;
    }
    (to.start + place).min(to.end - 1)
}

/// A folder as a message names it: by its own name, however deep or on whichever card.
fn folder(dir: &std::path::Path) -> String {
    dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
