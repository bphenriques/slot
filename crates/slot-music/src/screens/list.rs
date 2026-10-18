//! A list of rows, as every screen but Now Playing is: the library's lists here, and the main
//! menu, settings and queue on top of it.

use slot_store::initial;

use super::{Body, Legend, ListBody, Screen, Target};
use crate::browse::keys::Mode;
use crate::browse::rows::{self, Row};
use crate::browse::{per_page, Browser, Context, Key, Outcome, Shuffle, View};
use crate::text::{self, Notice};
use crate::Music;

/// Held, these move through rows, pages and letters, or carry a queued track.
pub(crate) fn repeats(key: Key) -> bool {
    matches!(
        key,
        Key::Up | Key::Down | Key::Left | Key::Right | Key::L1 | Key::R1
    )
}

/// A waits to see whether it is held on a row with songs to put next; the moving keys repeat.
pub(crate) fn mode(cx: &Context, view: View, row: Row, key: Key) -> Mode {
    match key {
        Key::A if rows::tracks(cx, view, row).is_some() => Mode::Hold,
        key if repeats(key) => Mode::Repeat,
        _ => Mode::Once,
    }
}

/// A held puts the row's songs next.
pub(crate) fn held(cx: &Context, view: View, row: Row, key: Key) -> Outcome {
    match (key, rows::tracks(cx, view, row)) {
        (Key::A, Some(tracks)) => Outcome::PlayNext(tracks),
        _ => Outcome::None,
    }
}

/// Up and Down go round; Left and Right turn a page; the shoulders jump a letter; A opens or
/// plays; X adds the row's songs to the end of the queue; Y puts a song in Favorites or takes
/// it out.
pub(crate) fn press(b: &mut Browser, cx: &Context, rows: &[Row], at: usize, key: Key) -> Outcome {
    let (n, view) = (rows.len(), b.view());
    // The queue has no letters to jump to: its shoulders turn pages too.
    let key = match (view, key) {
        (View::Queue, Key::L1) => Key::Left,
        (View::Queue, Key::R1) => Key::Right,
        _ => key,
    };
    let to = match key {
        Key::Up => (at + n - 1) % n,
        Key::Down => (at + 1) % n,
        Key::Left => at.saturating_sub(per_page(view, &cx.settings)),
        Key::Right => (at + per_page(view, &cx.settings)).min(n - 1),
        Key::L1 => rows::letter(cx.lib, rows, at, false),
        Key::R1 => rows::letter(cx.lib, rows, at, true),
        Key::X => return rows::tracks(cx, view, rows[at]).map_or(Outcome::None, Outcome::Enqueue),
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
        // With nothing to resume, it shuffles every song.
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
        Row::Artists => (View::Artists, Outcome::None),
        Row::Albums => (View::Albums, Outcome::None),
        Row::Tracks => (View::Tracks, Outcome::None),
        Row::Settings => (View::Settings, Outcome::None),
        Row::Sound => (View::Sound, Outcome::None),
        Row::BookSettings => (View::BookSettings, Outcome::None),
        Row::Debug => (View::Debug, Outcome::None),
        Row::TestBuzz => return Outcome::TestBuzz,
        Row::LastError => return Outcome::None,
        Row::Playlists => (View::Playlists, Outcome::None),
        Row::Playlist(p) => (View::Playlist(p), Outcome::None),
        Row::RecentlyAdded => (View::RecentlyAdded, Outcome::None),
        Row::RecentlyPlayed => (View::RecentlyPlayed, Outcome::None),
        Row::Books => (View::Books, Outcome::None),
        // A book plays on where it was left.
        Row::Book(book) => (View::NowPlaying, Outcome::PlayBook { book, over: false }),
        Row::Artist(a) => (View::Artist(a), Outcome::None),
        Row::Album(a) => (View::Album(a), Outcome::None),
        Row::Rescan => return Outcome::Rescan(false),
        // Rebuild acts only when A is held, as it takes long.
        Row::Rebuild => return Outcome::Tell(Notice::Hold("A")),
        Row::Setting(s) => return Outcome::Change(s, 0),
        // The START menus answer these themselves.
        Row::StartOver
        | Row::Finished
        | Row::Chapters
        | Row::Bookmarks
        | Row::Chapter(_)
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
        Row::Track(_) => {
            let tracks = match view {
                View::Album(a) => cx.lib.albums[a].tracks.clone(),
                View::Tracks => cx.lib.tracks_by_title.clone(),
                View::Playlist(p) => cx.lib.playlists[p].tracks.clone(),
                _ => return Outcome::None,
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

/// A library list, once the library is read.
pub(crate) fn screen(music: &Music) -> Screen {
    if !music.scan.ready {
        let (done, of) = music.reading();
        Screen::message(text::MUSIC, &text::reading(done, of))
    } else if music.library.tracks.is_empty() {
        Screen::message(text::MUSIC, text::NO_MUSIC)
    } else {
        rows_screen(music, |row| {
            Legend::List(match row {
                Some(Row::Track(_)) => Target::Song,
                Some(Row::Album(_)) => Target::Album,
                Some(Row::Artist(_)) => Target::Artist,
                Some(Row::Playlist(_) | Row::RecentlyAdded | Row::RecentlyPlayed) => {
                    Target::Playlist
                }
                Some(Row::Book(_)) => Target::Book,
                _ => Target::All,
            })
        })
    }
}

/// The page the cursor is on, with the hints `legend` gives for its highlighted row. Lists
/// count their items in the corner; menus do not.
pub(crate) fn rows_screen(music: &Music, legend: impl FnOnce(Option<Row>) -> Legend) -> Screen {
    let view = music.browser.view();
    let page = music.browser.page(&music.context());
    let legend = legend(page.highlighted);
    let count = match (view.is_menu(), page.item) {
        (true, _) => String::new(),
        (false, Some(i)) => text::of(i, page.items),
        (false, None) => page.items.to_string(),
    };
    // A menu on one page is centred on its rows; over pages, on a full page, so it holds still.
    let per_page = match view.is_menu() && page.pages == 1 {
        true => page.rows.len(),
        false => page.per_page,
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
    };
    Screen {
        corner: music.notice().unwrap_or(count),
        ..Screen::new(page.title, legend, Body::List(list))
    }
}
