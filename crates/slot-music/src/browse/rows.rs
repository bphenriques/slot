//! The rows of each list: what they are, what they say, and the tracks they stand for.

use crate::library::sort::{initial, sort_key};

use crate::books;
use crate::browse::{BookList, Context, QueueView, View};
use crate::library::Library;
use crate::library::{AlbumId, ArtistId, BookId, TrackId};
use crate::settings::Setting;
use crate::text;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Row {
    NowPlaying,
    Playlists,
    Playlist(usize),
    RecentlyAdded,
    /// The songs played last, newest first.
    RecentlyPlayed,
    /// Adds the track playing to Favorites, or takes it out.
    Favorite,
    Artists,
    Albums,
    Tracks,
    Settings,
    /// Music Settings' Sound page, and its Audiobooks page.
    Sound,
    BookSettings,
    Rescan,
    Rebuild,
    /// The Debug page, and what it holds besides a setting.
    Debug,
    LastError,
    Setting(Setting),
    PlayAll,
    Shuffle,
    Artist(ArtistId),
    Album(AlbumId),
    Track(TrackId),
    /// A place in the queue.
    Queued(usize),
    /// The audiobooks, and a book; its menu starts it over, or marks it finished or not. The
    /// audiobooks' menu plays on the latest book, or lists them, or their authors.
    Books,
    Book(BookId),
    Continue,
    BookList(BookList),
    Authors,
    Author(ArtistId),
    StartOver,
    Finished,
    /// A book's chapters and bookmarks, from its menu, and one of each; and its speed there.
    Chapters,
    Bookmarks,
    Chapter(usize),
    Bookmark(usize),
    Speed,
    /// Back to where the book was before it last moved.
    Back,
    GoToAlbum,
    GoToArtist,
    SongInfo,
    /// Now Playing's shuffle, on or off, as a row of its menu.
    ShuffleQueue,
    /// The queue's menu: keep it as a playlist, or take out all but what plays.
    SaveQueue,
    ClearQueue,
    Info(Info),
}

/// What Song Info tells of the track playing.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Info {
    Title,
    Artist,
    Album,
    Number,
    Format,
    File,
}

impl Info {
    const ALL: [Info; 6] = [
        Info::Title,
        Info::Artist,
        Info::Album,
        Info::Number,
        Info::Format,
        Info::File,
    ];
}

/// A row: its name, context before (a track number) or after (the artist), an icon, and
/// its state.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RowText {
    pub icon: Option<RowIcon>,
    pub before: String,
    pub text: String,
    pub after: String,
    pub playing: bool,
    /// Set back: played already, or nothing to resume.
    pub faint: bool,
    /// Acts on the whole list, like Play All.
    pub action: bool,
    /// A song in Favorites, marked with a heart at the right.
    pub favorite: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RowIcon {
    Play,
}

impl RowText {
    pub fn plain(text: impl Into<String>) -> RowText {
        RowText {
            text: text.into(),
            ..RowText::default()
        }
    }
}

/// A row compares by its name.
impl PartialEq<&str> for RowText {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

/// The action rows at the top of a list.
pub(crate) fn fixed(view: View) -> &'static [Row] {
    match view {
        View::Artists
        | View::Albums
        | View::Tracks
        | View::Artist(_)
        | View::Album(_)
        | View::Playlist(_)
        | View::RecentlyAdded
        | View::RecentlyPlayed => &[Row::PlayAll, Row::Shuffle],
        _ => &[],
    }
}

pub fn rows(cx: &Context, view: View) -> Vec<Row> {
    let (lib, queue) = (cx.lib, cx.queue);
    let mut rows = fixed(view).to_vec();
    match view {
        View::Main => {
            rows.extend([Row::NowPlaying, Row::Playlists]);
            rows.extend((!lib.books.is_empty()).then_some(Row::Books));
            rows.extend([Row::Artists, Row::Albums, Row::Tracks, Row::Settings]);
        }
        // The book to continue and those being listened to, when there are; then by author, all,
        // and those finished, once there are.
        View::Books => {
            let any = |list| !books::listed(lib, cx.shelf, list).is_empty();
            rows.extend(any(BookList::Listening).then_some(Row::Continue));
            rows.extend(any(BookList::Listening).then_some(Row::BookList(BookList::Listening)));
            rows.extend([Row::Authors, Row::BookList(BookList::All)]);
            rows.extend(any(BookList::Finished).then_some(Row::BookList(BookList::Finished)));
        }
        View::BookList(list) => rows.extend(
            books::listed(lib, cx.shelf, list)
                .into_iter()
                .map(Row::Book),
        ),
        View::Authors => rows.extend(books::authors(lib).into_iter().map(Row::Author)),
        View::Author(a) => rows.extend(books::by(lib, a).into_iter().map(Row::Book)),
        // The file's own chapters, or else the book's parts.
        View::Chapters => match queue.chapters.len() {
            0 => rows.extend((0..queue.tracks.len()).map(Row::Chapter)),
            n => rows.extend((0..n).map(Row::Chapter)),
        },
        View::Bookmarks(book) => {
            let n = cx.shelf.bookmarks(lib, book).len();
            rows.extend((0..n).map(Row::Bookmark));
        }
        // Listening first, then the Sound and Audiobooks pages, then looks.
        View::Settings => {
            let (listening, looks) = Setting::ALL.split_at(3);
            rows.extend(listening.iter().copied().map(Row::Setting));
            rows.push(Row::Sound);
            rows.extend((!lib.books.is_empty()).then_some(Row::BookSettings));
            rows.extend(looks.iter().copied().map(Row::Setting));
            rows.extend([Row::Rescan, Row::Rebuild, Row::Debug]);
        }
        View::Sound => rows.extend(Setting::SOUND.map(Row::Setting)),
        View::BookSettings => rows.extend(Setting::BOOKS.map(Row::Setting)),
        View::Debug => rows.extend([Row::Setting(Setting::PerfLog), Row::LastError]),
        View::Queue => rows.extend((0..queue.tracks.len()).map(Row::Queued)),
        View::Artists => rows.extend(lib.music_artists.iter().map(|&a| Row::Artist(a))),
        View::Artist(a) => rows.extend(lib.artists[a].albums.iter().map(|&a| Row::Album(a))),
        View::Albums => rows.extend(lib.albums_by_name.iter().map(|&a| Row::Album(a))),
        View::Album(a) => rows.extend(lib.albums[a].tracks.iter().map(|&t| Row::Track(t))),
        View::Tracks => rows.extend(lib.tracks_by_title.iter().map(|&t| Row::Track(t))),
        // Favorites, Recently Played and Recently Added first, as players pin them; then the
        // files by name, one saved since the library was read among them.
        View::Playlists => {
            let mut lists: Vec<usize> = (1..lib.playlists.len()).collect();
            lists.sort_by_cached_key(|&p| sort_key(&lib.playlists[p].name));
            rows.extend((!lib.playlists.is_empty()).then_some(Row::Playlist(0)));
            rows.extend([Row::RecentlyPlayed, Row::RecentlyAdded]);
            rows.extend(lists.into_iter().map(Row::Playlist));
        }
        View::Playlist(p) => rows.extend(lib.playlists[p].tracks.iter().map(|&t| Row::Track(t))),
        View::RecentlyAdded => rows.extend(lib.albums_by_added.iter().map(|&a| Row::Album(a))),
        View::RecentlyPlayed => rows.extend(cx.recent.iter().map(|&t| Row::Track(t))),
        // A book's: where to go in it, then how it plays, as players offer them.
        View::PlayingMenu if cx.book.is_some() => {
            // Back first, as it is reached for right after a slip.
            rows.extend(cx.back.map(|_| Row::Back));
            rows.extend([
                Row::Chapters,
                Row::Bookmarks,
                Row::Speed,
                Row::Setting(Setting::SleepTimer),
                Row::Finished,
            ]);
        }
        // What to do with the song first, then where to go from it.
        View::PlayingMenu => rows.extend([
            Row::Favorite,
            Row::Setting(Setting::SleepTimer),
            Row::Setting(Setting::Repeat),
            Row::ShuffleQueue,
            Row::GoToAlbum,
            Row::GoToArtist,
            Row::SongInfo,
        ]),
        View::QueueMenu => rows.extend([Row::SaveQueue, Row::ClearQueue]),
        // Only what no key does: A, X and Y play, queue and favorite from the list itself.
        View::RowMenu(Row::Track(_)) => {
            rows.extend([Row::GoToAlbum, Row::GoToArtist, Row::SongInfo])
        }
        View::RowMenu(_) => rows.extend([Row::StartOver, Row::Finished]),
        View::SongInfo(_) => rows.extend(Info::ALL.map(Row::Info)),
        View::NowPlaying | View::Lyrics => {}
    }
    rows
}

/// A row as shown. The playing track shows the play symbol in place of its number.
pub(crate) fn label(cx: &Context, view: View, row: Row) -> RowText {
    let mut text = describe(cx, view, row);
    if text.playing {
        text.icon = Some(RowIcon::Play);
        text.before.clear();
    }
    text.action = fixed(view).contains(&row);
    text
}

fn describe(cx: &Context, view: View, row: Row) -> RowText {
    let (lib, queue) = (cx.lib, cx.queue);
    let current = queue.tracks.get(queue.at).copied();
    let subject = subject(queue, view);
    let track = |t: TrackId| RowText {
        text: lib.tracks[t].title.clone(),
        after: lib.tracks[t].artist.clone(),
        playing: Some(t) == current,
        favorite: lib.is_favorite(t),
        ..RowText::default()
    };
    match row {
        // Now Playing, or Resume while paused, as Rockbox has it; with nothing to resume, a
        // shuffle of everything. A saved queue still being read back is something to resume.
        Row::NowPlaying => match (idle(queue), queue.playing) {
            (true, _) => RowText::plain(text::SHUFFLE_LIBRARY),
            (false, true) => RowText::plain(text::NOW_PLAYING),
            (false, false) => RowText::plain(text::RESUME),
        },
        Row::Artist(a) => RowText::plain(&lib.artists[a].name),
        Row::Author(a) => RowText::plain(&lib.artists[a].name),
        // Continue lit, as an action is, then the book it plays on, dim.
        Row::Continue => RowText {
            after: books::latest(lib, cx.shelf)
                .map(|b| lib.albums[lib.books[b].album].name.clone())
                .unwrap_or_default(),
            ..RowText::plain(text::CONTINUE)
        },
        // Favorites is worded here; the rest are named by their files.
        Row::Playlist(0) => RowText::plain(text::FAVORITES),
        Row::Playlist(p) => RowText::plain(&lib.playlists[p].name),
        Row::Favorite => RowText::plain(match subject.is_some_and(|t| lib.is_favorite(t)) {
            true => text::UNFAVORITE,
            false => text::FAVORITE,
        }),
        // The menu names where it goes: "Go to" lit, as the other actions are, the name dim.
        Row::GoToAlbum | Row::GoToArtist => match (row, subject) {
            (_, None) => RowText::plain(text::row(row, view).unwrap_or_default()),
            (row, Some(t)) => RowText {
                after: match row {
                    Row::GoToAlbum => &lib.album_of(t).name,
                    _ => &lib.artist_of(t).name,
                }
                .clone(),
                ..RowText::plain(text::GO_TO)
            },
        },
        // Albums from across the library name their artist; an artist's own need not.
        Row::Album(a) if matches!(view, View::Albums | View::RecentlyAdded) => RowText {
            after: lib.artists[lib.albums[a].artist].name.clone(),
            ..RowText::plain(&lib.albums[a].name)
        },
        Row::Album(a) => RowText::plain(&lib.albums[a].name),
        Row::Book(b) => {
            let album = &lib.albums[lib.books[b].album];
            RowText {
                after: lib.artists[album.artist].name.clone(),
                ..RowText::plain(&album.name)
            }
        }
        // The one playing marked; those before, faint, as in the queue.
        Row::Chapter(i) => {
            let playing = queue.chapter.unwrap_or(queue.at);
            let title = match queue.chapters.get(i) {
                Some(c) => c.title.clone(),
                None => lib.tracks[queue.tracks[i]].title.clone(),
            };
            RowText {
                playing: i == playing,
                faint: i < playing,
                ..RowText::plain(title)
            }
        }
        // Where it is, then its chapter.
        Row::Bookmark(i) => {
            let View::Bookmarks(book) = view else {
                return RowText::default();
            };
            let mark = &cx.shelf.bookmarks(lib, book)[i];
            RowText {
                before: text::clock(mark.pos),
                ..RowText::plain(&mark.chapter)
            }
        }
        Row::Back => RowText::plain(text::back_to(cx.back.unwrap_or_default())),
        // A book's menu offers the other way.
        Row::Finished => {
            let book = match view {
                View::RowMenu(Row::Book(b)) => Some(b),
                _ => cx.book,
            };
            let finished = book.is_some_and(|b| cx.shelf.place(lib, b).finished);
            RowText::plain(text::finished(finished))
        }
        // Tracks show their number in their album, their artist elsewhere.
        Row::Track(t) if view == View::Album(lib.tracks[t].album) => RowText {
            before: lib.tracks[t].number.clone(),
            after: String::new(),
            ..track(t)
        },
        Row::Track(t) => track(t),
        Row::Queued(i) => RowText {
            playing: i == queue.at,
            faint: i < queue.at,
            ..track(queue.tracks[i])
        },
        // What it is, lit as a label, then what the track's is, dim.
        Row::Info(info) => RowText {
            after: subject.map(|t| fact(lib, t, info)).unwrap_or_default(),
            ..RowText::plain(text::info(info))
        },
        row => RowText::plain(text::row(row, view).unwrap_or_default()),
    }
}

/// One thing known of track `t`, from its file and folders.
fn fact(lib: &Library, t: TrackId, info: Info) -> String {
    let track = &lib.tracks[t];
    match info {
        Info::Title => track.title.clone(),
        Info::Artist => lib.tracks[t].artist.clone(),
        Info::Album => lib.album_of(t).name.clone(),
        Info::Number => track.number.clone(),
        Info::Format => track
            .path
            .extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_default(),
        Info::File => track
            .path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// A setting's choice, or a main-screen count.
pub(crate) fn value(cx: &Context, row: Row) -> Option<String> {
    let count = |n: usize| (n > 0).then(|| n.to_string());
    match row {
        Row::Artists => count(cx.lib.music_artists.len()),
        Row::Albums => count(cx.lib.albums_by_name.len()),
        Row::Books => count(cx.lib.books.len()),
        Row::BookList(list) => count(books::listed(cx.lib, cx.shelf, list).len()),
        Row::Authors => count(books::authors(cx.lib).len()),
        Row::Author(a) => count(books::by(cx.lib, a).len()),
        // How far into a book, once begun.
        Row::Book(b) => {
            let place = cx.shelf.place(cx.lib, b);
            let done = books::progress(cx.lib, b, &place);
            match (place.finished, done > 0.0) {
                (true, _) => Some(text::FINISHED.into()),
                (false, true) => Some(text::percent(done)),
                (false, false) => None,
            }
        }
        // How long a chapter is.
        Row::Chapter(i) => {
            // A chapter runs to the next, the last to the end of its part.
            let q = cx.queue;
            let secs = |t: usize| Some(cx.lib.tracks[*q.tracks.get(t)?].secs).filter(|&s| s > 0.0);
            let len = match q.chapters.get(i) {
                Some(c) => q
                    .chapters
                    .get(i + 1)
                    .map(|c| c.start)
                    .or(secs(q.at))
                    .map(|end| end - c.start),
                None => secs(i),
            };
            len.map(text::clock)
        }
        Row::Tracks => count(cx.lib.tracks_by_title.len()),
        Row::Playlist(p) => count(cx.lib.playlists[p].tracks.len()),
        Row::Setting(s) => Some(text::setting(s, &cx.settings, cx.sleep, cx.book.is_some())),
        Row::Speed => Some(text::speed(cx.queue.speed)),
        Row::ShuffleQueue => Some(text::on_off(cx.shuffled).into()),
        Row::LastError => Some(cx.last_error.unwrap_or(text::NONE).to_string()),
        _ => None,
    }
}

/// The tracks a row stands for, in `view`.
pub(crate) fn tracks(cx: &Context, view: View, row: Row) -> Option<Vec<TrackId>> {
    let lib = cx.lib;
    Some(match row {
        Row::Artist(a) => lib.artist_tracks(a),
        Row::Album(a) => lib.albums[a].tracks.clone(),
        Row::Track(t) => vec![t],
        Row::Playlist(p) => lib.playlists[p].tracks.clone(),
        Row::RecentlyPlayed => cx.recent.to_vec(),
        Row::RecentlyAdded => albums_tracks(lib, &lib.albums_by_added),
        Row::PlayAll | Row::Shuffle => match view {
            View::Artist(a) => lib.artist_tracks(a),
            View::Album(a) => lib.albums[a].tracks.clone(),
            View::Tracks => lib.tracks_by_title.clone(),
            View::Artists => lib.music_tracks(),
            View::Albums => albums_tracks(lib, &lib.albums_by_name),
            View::Playlist(p) => lib.playlists[p].tracks.clone(),
            View::RecentlyPlayed => cx.recent.to_vec(),
            View::RecentlyAdded => albums_tracks(lib, &lib.albums_by_added),
            _ => return None,
        },
        _ => return None,
    })
}

/// The song a START menu is about: the one picked in a list, the one Song Info tells of, or
/// else the one playing.
pub(crate) fn subject(queue: QueueView, view: View) -> Option<TrackId> {
    match view {
        View::RowMenu(Row::Track(t)) | View::SongInfo(t) => Some(t),
        _ => queue.tracks.get(queue.at).copied(),
    }
}

/// Whether there is nothing to play on with: no queue, and none being read back.
pub(crate) fn idle(queue: QueueView) -> bool {
    queue.tracks.is_empty() && !queue.waiting
}

/// The tracks of `albums`, one album after another.
fn albums_tracks(lib: &Library, albums: &[AlbumId]) -> Vec<TrackId> {
    albums
        .iter()
        .flat_map(|&a| lib.albums[a].tracks.iter().copied())
        .collect()
}

/// How a list's tracks split into the albums or artists it shows.
pub(crate) fn groups(lib: &Library, view: View) -> Option<Vec<usize>> {
    let albums = |ids: &[AlbumId]| ids.iter().map(|&a| lib.albums[a].tracks.len()).collect();
    match view {
        View::Artists => Some(
            lib.artists
                .iter()
                .map(|a| a.albums.iter().map(|&b| lib.albums[b].tracks.len()).sum())
                .collect(),
        ),
        View::Albums => Some(albums(&lib.albums_by_name)),
        View::RecentlyAdded => Some(albums(&lib.albums_by_added)),
        View::Artist(a) => Some(albums(&lib.artists[a].albums)),
        _ => None,
    }
}

pub(crate) fn name(lib: &Library, row: Row) -> Option<&str> {
    match row {
        Row::Artist(a) => Some(&lib.artists[a].name),
        Row::Album(a) => Some(&lib.albums[a].name),
        Row::Track(t) => Some(&lib.tracks[t].title),
        Row::Author(a) => Some(&lib.artists[a].name),
        Row::Book(b) => Some(&lib.albums[lib.books[b].album].name),
        _ => None,
    }
}

/// The first row of the previous or next letter, grouped as on the shelf. Action rows are
/// their own group.
pub(crate) fn letter(lib: &Library, rows: &[Row], at: usize, forward: bool) -> usize {
    let group = |i: usize| name(lib, rows[i]).map(initial);
    let here = group(at);
    let start = |mut i: usize| {
        while i > 0 && group(i - 1) == group(i) {
            i -= 1;
        }
        i
    };
    match forward {
        true => (at + 1..rows.len())
            .find(|&i| group(i) != here)
            .unwrap_or(at),
        false => match start(at) {
            0 => 0,
            first => start(first - 1),
        },
    }
}
