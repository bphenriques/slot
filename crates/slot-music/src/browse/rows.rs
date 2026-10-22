//! The rows of each list: what they are, what they say, and the tracks they stand for.

use crate::library::sort::{initial, sort_key};

use crate::books;
use crate::books::chapters::Chapter;
use crate::browse::{BookList, Context, QueueView, View};
use crate::library::Library;
use crate::library::{AlbumId, ArtistId, BookId, FolderId, GenreId, TrackId};
use crate::settings::Setting;
use crate::store::equalizer::BANDS;
use crate::text;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Row {
    NowPlaying,
    /// Opens a page named as it is: Library, Playlists, Settings, the audiobooks' lists, and the
    /// rest that only lead somewhere.
    Open(View),
    /// A genre, and a decade.
    Genre(GenreId),
    Folder(FolderId),
    Decade(u32),
    Playlist(usize),
    Rescan,
    Setting(Setting),
    PlayAll,
    Shuffle,
    Artist(ArtistId),
    Album(AlbumId),
    Track(TrackId),
    /// A place in the queue.
    Queued(usize),
    /// A book; its menu starts it over, or marks it finished or not. The audiobooks' menu plays
    /// on the latest book.
    Book(BookId),
    Continue,
    Author(ArtistId),
    StartOver,
    Finished,
    /// A book's chapters and bookmarks, from its menu, and one of each; and its speed there.
    Chapters,
    Bookmarks,
    Bookmark(usize),
    /// A chapter of a book listed from the books: its `part`'s `chapter`, or the part, for one
    /// with none.
    BookChapter {
        book: BookId,
        part: usize,
        chapter: Option<usize>,
    },
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
    /// A line of Help, to read.
    HelpLine(usize),
    /// An equalizer preset, from 1, or Off for 0; New Preset; and one of its bands.
    Preset(usize),
    NewPreset,
    Band(usize),
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

/// A row: its name, context before (a track number) or after (the artist), and its state.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RowText {
    pub before: String,
    pub text: String,
    pub after: String,
    /// The one playing, the play symbol in place of what goes before it.
    pub playing: bool,
    /// Set back: played already, or nothing to resume.
    pub faint: bool,
    /// Acts on the whole list, like Play All.
    pub action: bool,
    /// A song in Favorites, marked with a heart at the right.
    pub favorite: bool,
    /// The album whose thumbnail starts it, once read.
    pub album: Option<AlbumId>,
    /// A sentence to read, set as written rather than in capitals: Help's tips.
    pub cased: bool,
    /// Its value is a name, set as written as names are: what plays, beside Now Playing.
    pub named: bool,
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

/// The action row at the top of a list: Play All, which held shuffles, as one row in every list.
pub(crate) fn fixed(view: View) -> &'static [Row] {
    match view {
        View::Artists
        | View::Albums
        | View::Tracks
        | View::Artist(_)
        | View::Album(_)
        | View::Playlist(_)
        | View::Folder(_)
        | View::Genre(_)
        | View::Decade(_)
        | View::RecentlyAdded
        | View::RecentlyPlayed => &[Row::PlayAll],
        _ => &[],
    }
}

pub fn rows(cx: &Context, view: View) -> Vec<Row> {
    let (lib, queue) = (cx.lib, cx.queue);
    let mut rows = fixed(view).to_vec();
    match view {
        View::Main => {
            rows.extend([
                Row::NowPlaying,
                Row::Open(View::Queue),
                Row::Open(View::Library),
                Row::Open(View::Playlists),
                Row::Open(View::Books),
                Row::Open(View::Settings),
            ]);
        }
        // Genres and Years only once the tags fill them, and Folders once there are songs.
        View::Library => {
            rows.extend([
                Row::Open(View::Artists),
                Row::Open(View::Albums),
                Row::Open(View::Tracks),
            ]);
            rows.extend((!lib.genres.is_empty()).then_some(Row::Open(View::Genres)));
            rows.extend((!lib.albums_by_year.is_empty()).then_some(Row::Open(View::Years)));
            let music = lib.folders.first();
            let filled = music.is_some_and(|m| !m.folders.is_empty() || !m.tracks.is_empty());
            rows.extend(filled.then_some(Row::Open(View::Folder(FolderId(0)))));
        }
        View::Genres => rows.extend((0..lib.genres.len()).map(|g| Row::Genre(GenreId(g)))),
        View::Genre(g) => rows.extend(lib.genres[g].albums.iter().map(|&a| Row::Album(a))),
        View::Folder(f) => {
            let folder = &lib.folders[f];
            rows.extend(folder.folders.iter().map(|&f| Row::Folder(f)));
            rows.extend(folder.tracks.iter().map(|&t| Row::Track(t)));
        }
        View::Years => {
            let mut decades: Vec<u32> =
                lib.albums_by_year.iter().map(|&a| decade(lib, a)).collect();
            decades.dedup();
            rows.extend(decades.into_iter().map(Row::Decade));
        }
        View::Decade(d) => rows.extend(in_decade(lib, d).iter().map(|&a| Row::Album(a))),
        // The book to continue and those being listened to, when there are; then by author, all,
        // and those finished, once there are.
        View::Books => {
            let any = |list| !books::listed(lib, cx.shelf, list).is_empty();
            rows.extend(any(BookList::Listening).then_some(Row::Continue));
            rows.extend(
                any(BookList::Listening).then_some(Row::Open(View::BookList(BookList::Listening))),
            );
            rows.extend([
                Row::Open(View::Authors),
                Row::Open(View::BookList(BookList::All)),
            ]);
            rows.extend(
                any(BookList::Finished).then_some(Row::Open(View::BookList(BookList::Finished))),
            );
        }
        View::BookList(list) => rows.extend(
            books::listed(lib, cx.shelf, list)
                .into_iter()
                .map(Row::Book),
        ),
        View::Authors => rows.extend(books::authors(lib).into_iter().map(Row::Author)),
        View::Author(a) => rows.extend(books::by(lib, a).into_iter().map(Row::Book)),
        View::BookChapters(book) => rows.extend(book_chapters(cx, book)),
        View::Bookmarks(book) => {
            let n = cx.shelf.bookmarks(lib, book).len();
            rows.extend((0..n).map(Row::Bookmark));
        }
        // The Sound and Audiobooks pages, the settings on this one, the library, then the pages
        // seldom wanted.
        View::Settings => {
            rows.push(Row::Open(View::MusicSettings));
            rows.extend((!lib.books.is_empty()).then_some(Row::Open(View::BookSettings)));
            rows.extend(Setting::ALL.map(Row::Setting));
            rows.extend([
                Row::Rescan,
                Row::Open(View::Help),
                Row::Open(View::Advanced),
            ]);
        }
        View::MusicSettings => {
            rows.extend(Setting::MUSIC.map(Row::Setting));
            rows.push(Row::Open(View::Equalizer));
        }
        // Off, then the presets, slot's and one's own; then one to make.
        View::Equalizer => {
            rows.extend((0..=cx.equalizer.len()).map(Row::Preset));
            rows.push(Row::NewPreset);
        }
        View::NewPreset => rows.extend((0..BANDS).map(Row::Band)),
        View::BookSettings => rows.extend(Setting::BOOKS.map(Row::Setting)),
        View::Experimental => rows.extend(Setting::EXPERIMENTAL.map(Row::Setting)),
        // What tells about the player: the scrobbler log for an uploader, the detailed log, and
        // the performance log only in a build that measures (`--features perf`).
        View::Advanced => {
            rows.push(Row::Setting(Setting::Scrobbler));
            rows.extend(cfg!(feature = "perf").then_some(Row::Setting(Setting::PerfLog)));
            rows.push(Row::Setting(Setting::Log));
            rows.push(Row::Open(View::Experimental));
        }
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
            rows.extend([
                Row::Open(View::RecentlyPlayed),
                Row::Open(View::RecentlyAdded),
            ]);
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
            Row::Setting(Setting::SleepTimer),
            Row::Setting(Setting::Repeat),
            Row::ShuffleQueue,
            Row::GoToAlbum,
            Row::GoToArtist,
            Row::SongInfo,
        ]),
        View::QueueMenu => rows.extend([Row::SaveQueue, Row::ClearQueue]),
        // Only what no key does: A, X and Y play, queue and favorite from the list itself.
        View::SongMenu(_) => rows.extend([Row::GoToAlbum, Row::GoToArtist, Row::SongInfo]),
        View::BookMenu(_) => rows.extend([Row::Chapters, Row::StartOver, Row::Finished]),
        View::SongInfo(_) => rows.extend(Info::ALL.map(Row::Info)),
        // Now Playing, or text to read: no rows.
        View::Help => rows.extend((0..text::help_len()).map(Row::HelpLine)),
        View::NowPlaying | View::Lyrics => {}
    }
    rows
}

/// A book's chapters: each part's own, as read from its file, or else the part itself.
fn book_chapters(cx: &Context, book: BookId) -> Vec<Row> {
    let parts = &cx.lib.albums[cx.lib.books[book].album].tracks;
    let mut rows = Vec::new();
    for (part, &t) in parts.iter().enumerate() {
        let row = |chapter| Row::BookChapter {
            book,
            part,
            chapter,
        };
        match cx.chapters.of(t).map_or(0, Vec::len) {
            0 => rows.push(row(None)),
            n => rows.extend((0..n).map(|i| row(Some(i)))),
        }
    }
    rows
}

/// Where a chapter comes in its book, from 1, through all its parts: as an album numbers its
/// songs, so the play mark takes the number's place and the names stay in line.
fn chapter_number(cx: &Context, book: BookId, part: usize, chapter: Option<usize>) -> usize {
    let before: usize = (0..part)
        .map(|p| part_chapters(cx, book, p).len().max(1))
        .sum();
    before + chapter.unwrap_or(0) + 1
}

/// The chapters of `book`'s `part` read from its file; none until they are, or if it has none.
pub(crate) fn part_chapters<'a>(cx: &Context<'a>, book: BookId, part: usize) -> &'a [Chapter] {
    let parts = &cx.lib.albums[cx.lib.books[book].album].tracks;
    parts
        .get(part)
        .and_then(|&t| cx.chapters.of(t))
        .map_or(&[], Vec::as_slice)
}

/// Where in its part a book's chapter starts, in seconds; a part with none, at its start.
pub(crate) fn chapter_start(
    cx: &Context,
    book: BookId,
    part: usize,
    chapter: Option<usize>,
) -> f64 {
    chapter
        .and_then(|i| part_chapters(cx, book, part).get(i))
        .map_or(0.0, |c| c.start)
}

/// The chapter `book` was left in, as its part and its chapter there: where its list opens.
fn left_in(cx: &Context, book: BookId) -> (usize, Option<usize>) {
    let place = cx.shelf.place(cx.lib, book);
    let chapters = part_chapters(cx, book, place.part);
    let chapter = chapters.iter().rposition(|c| c.start <= place.pos);
    (place.part, chapter.or((!chapters.is_empty()).then_some(0)))
}

/// The row of `rows` (a book's chapters) it was left in, to open the list at.
pub(crate) fn left_at(cx: &Context, rows: &[Row], book: BookId) -> usize {
    let left = left_in(cx, book);
    rows.iter()
        .position(
            |&r| matches!(r, Row::BookChapter { part, chapter, .. } if (part, chapter) == left),
        )
        .unwrap_or(0)
}

/// The album whose thumbnail starts `row` in `view`: an album's or a book's own, or a song's.
/// Only in a list with thumbnails (`View::rich`), the one place that says which have them; none in
/// an album's own list, say, which numbers its songs instead.
pub(crate) fn album(cx: &Context, view: View, row: Row) -> Option<AlbumId> {
    let lib = cx.lib;
    if !view.rich() {
        return None;
    }
    match row {
        Row::Album(a) => Some(a),
        Row::Book(b) => Some(lib.books[b].album),
        Row::Track(t) => Some(lib.tracks[t].album),
        Row::Queued(i) => cx.queue.tracks.get(i).map(|&t| lib.tracks[t].album),
        Row::Folder(f) => lib.folders[f].first.map(|t| lib.tracks[t].album),
        _ => None,
    }
}

/// What plays, as Now Playing's row names it beside: the song, or the book.
fn playing_title(cx: &Context) -> Option<String> {
    let lib = cx.lib;
    match (cx.book, cx.queue.tracks.get(cx.queue.at)) {
        (Some(book), _) => Some(book_title(lib, book)),
        (None, Some(&t)) => Some(lib.tracks[t].title.clone()),
        (None, None) => None,
    }
}

fn book_title(lib: &Library, book: BookId) -> String {
    lib.albums[lib.books[book].album].name.clone()
}

/// A row as shown. What plays is marked in its number's place, the number's room kept so its
/// name stays in line with the rest.
pub(crate) fn label(cx: &Context, view: View, row: Row) -> RowText {
    let mut text = describe(cx, view, row);
    text.action = fixed(view).contains(&row);
    // Its thumbnail, once read.
    text.album = album(cx, view, row).filter(|&a| cx.thumbs.iter().any(|t| t.album == a));
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
            (true, _) => RowText {
                faint: lib.tracks_by_title.is_empty(),
                ..RowText::plain(text::SHUFFLE_LIBRARY)
            },
            (false, playing) => RowText {
                named: true,
                ..RowText::plain(if playing {
                    text::NOW_PLAYING
                } else {
                    text::RESUME
                })
            },
        },
        Row::Artist(a) => RowText::plain(&lib.artists[a].name),
        Row::Author(a) => RowText::plain(&lib.artists[a].name),
        Row::Continue => RowText {
            named: true,
            ..RowText::plain(text::CONTINUE)
        },
        // Favorites is worded here; the rest are named by their files.
        Row::Playlist(0) => RowText::plain(text::FAVORITES),
        Row::Playlist(p) => RowText::plain(&lib.playlists[p].name),
        // Set back with nothing to list yet; open, they say where to put it, or that nothing is up
        // next.
        Row::Open(View::Queue) => RowText {
            faint: idle(queue),
            ..RowText::plain(text::QUEUE)
        },
        Row::Open(View::Library) | Row::Open(View::Playlists) => RowText {
            faint: lib.tracks_by_title.is_empty(),
            ..RowText::plain(text::row(row, view).unwrap_or_default())
        },
        Row::Open(View::Books) => RowText {
            faint: lib.books.is_empty(),
            ..RowText::plain(text::row(row, view).unwrap_or_default())
        },
        Row::Genre(g) => RowText::plain(&lib.genres[g].name),
        // A folder says how many songs it holds, however deep, below its name.
        Row::Folder(f) => RowText {
            after: text::songs(lib.folders[f].songs),
            ..RowText::plain(&lib.folders[f].name)
        },
        Row::Decade(d) => RowText::plain(text::decade(d)),
        // A decade's albums say who made them and when, below, as an artist's say when.
        Row::Album(a) if matches!(view, View::Decade(_)) => RowText {
            after: text::by_in(&lib.artists[lib.albums[a].artist].name, lib.albums[a].year),
            ..RowText::plain(&lib.albums[a].name)
        },
        // Albums from across the library name their artist; an artist's own need not.
        Row::Album(a) if matches!(view, View::Albums | View::RecentlyAdded | View::Genre(_)) => {
            RowText {
                after: lib.artists[lib.albums[a].artist].name.clone(),
                ..RowText::plain(&lib.albums[a].name)
            }
        }
        // An artist's own albums say when they came out, below.
        Row::Album(a) if matches!(view, View::Artist(_)) => RowText {
            after: match lib.albums[a].year {
                0 => String::new(),
                year => year.to_string(),
            },
            ..RowText::plain(&lib.albums[a].name)
        },
        Row::Album(a) => RowText::plain(&lib.albums[a].name),
        // The book playing marked, as a song playing is.
        Row::Book(b) => {
            let album = &lib.albums[lib.books[b].album];
            RowText {
                after: lib.artists[album.artist].name.clone(),
                playing: cx.book == Some(b),
                ..RowText::plain(&album.name)
            }
        }
        // The chapter a book was left in marked, once begun, those before faint, as the book
        // playing's are; titled by its file, or else by its part.
        Row::BookChapter {
            book,
            part,
            chapter,
        } => {
            let parts = &lib.albums[lib.books[book].album].tracks;
            let title = chapter
                .and_then(|i| part_chapters(cx, book, part).get(i))
                .map(|c| c.title.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| lib.tracks[parts[part]].title.clone());
            let place = cx.shelf.place(lib, book);
            let begun = place.when > 0 && !place.finished;
            let left = left_in(cx, book);
            RowText {
                playing: begun && (part, chapter) == left,
                faint: begun && (part, chapter) < left,
                before: format!("{:02}", chapter_number(cx, book, part, chapter)),
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
                View::BookMenu(b) => Some(b),
                _ => cx.book,
            };
            let finished = book.is_some_and(|b| cx.shelf.place(lib, b).finished);
            RowText::plain(text::finished(finished))
        }
        // Tracks show their number in their album, their artist elsewhere; in their album too,
        // under it, where it is not the album's, as on a compilation or a song with a guest.
        Row::Track(t) if view == View::Album(lib.tracks[t].album) => RowText {
            before: lib.tracks[t].number.clone(),
            after: Some(&lib.tracks[t].artist)
                .filter(|&by| *by != lib.artist_of(t).name)
                .cloned()
                .unwrap_or_default(),
            ..track(t)
        },
        // In a folder, a song goes by its file's name, as a file browser lists it.
        Row::Track(t) if matches!(view, View::Folder(_)) => {
            let path = &lib.tracks[t].path;
            let stem = path.file_stem().unwrap_or(path.as_os_str());
            RowText::plain(stem.to_string_lossy())
        }
        Row::Track(t) => track(t),
        Row::Queued(i) => RowText {
            playing: i == queue.at,
            faint: i < queue.at,
            ..track(queue.tracks[i])
        },
        // A tip lit, as written; a place's name dim, as a heading, not faint as what cannot be
        // picked is; a key, then what it does, dim.
        Row::HelpLine(i) => match text::help(i) {
            Some(text::HelpLine::Tip(tip)) => RowText {
                cased: true,
                ..RowText::plain(tip)
            },
            Some(text::HelpLine::Place(place)) => RowText {
                before: place.into(),
                ..RowText::default()
            },
            Some(text::HelpLine::Key(key, does)) => RowText {
                after: does.into(),
                ..RowText::plain(key)
            },
            None => RowText::default(),
        },
        // What it is, lit as a label, then what the track's is, dim.
        Row::Info(info) => RowText {
            after: subject.map(|t| fact(lib, t, info)).unwrap_or_default(),
            ..RowText::plain(text::info(info))
        },
        // The preset playing marked as a song playing is.
        Row::Preset(i) => RowText {
            playing: i == cx.settings.eq,
            ..RowText::plain(cx.equalizer.name(i).unwrap_or(text::on_off(false)))
        },
        Row::Band(band) => RowText::plain(text::hz(cx.equalizer.hz()[band])),
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
        Row::Open(View::Artists) => count(cx.lib.music_artists.len()),
        Row::Open(View::Albums) => count(cx.lib.albums_by_name.len()),
        // What plays, and the book to continue, beside the rows that go to them, as a setting's
        // value is: one line, as every row of a menu is.
        Row::NowPlaying => playing_title(cx),
        Row::Continue => books::latest(cx.lib, cx.shelf).map(|b| book_title(cx.lib, b)),
        Row::Open(View::Genres) => count(cx.lib.genres.len()),
        Row::Open(View::Years) => {
            let mut decades: Vec<u32> = cx
                .lib
                .albums_by_year
                .iter()
                .map(|&a| decade(cx.lib, a))
                .collect();
            decades.dedup();
            count(decades.len())
        }
        // As a file browser counts a folder: what is in it, folders and songs.
        Row::Open(View::Folder(f)) => {
            let folder = &cx.lib.folders[f];
            count(folder.folders.len() + folder.tracks.len())
        }
        Row::Genre(g) => count(cx.lib.genres[g].albums.len()),
        Row::Decade(d) => count(in_decade(cx.lib, d).len()),
        Row::Open(View::BookList(list)) => count(books::listed(cx.lib, cx.shelf, list).len()),
        Row::Open(View::Authors) => count(books::authors(cx.lib).len()),
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
        // How long a book's chapter is: to the next, the last to the end of its part.
        Row::BookChapter {
            book,
            part,
            chapter,
        } => {
            let parts = &cx.lib.albums[cx.lib.books[book].album].tracks;
            let secs = cx.lib.tracks[parts[part]].secs;
            let next = chapter.and_then(|i| part_chapters(cx, book, part).get(i + 1));
            let (start, end) = (
                chapter_start(cx, book, part, chapter),
                next.map_or(secs, |c| c.start),
            );
            (end > start).then(|| text::clock(end - start))
        }
        Row::Open(View::Tracks) => count(cx.lib.tracks_by_title.len()),
        Row::Playlist(p) => count(cx.lib.playlists[p].tracks.len()),
        Row::Setting(s) => Some(text::setting(s, &cx.settings, cx.sleep, cx.book.is_some())),
        // The preset chosen, beside the page that chooses it; a band, how far it goes.
        Row::Open(View::Equalizer) => Some(
            cx.equalizer
                .name(cx.settings.eq)
                .unwrap_or(text::on_off(false))
                .into(),
        ),
        Row::Band(band) => Some(text::db(cx.tuning[band])),
        Row::Speed => Some(text::speed(cx.queue.speed)),
        Row::ShuffleQueue => Some(text::on_off(cx.shuffled).into()),
        // How many songs the library holds, beside reading it again.
        Row::Rescan => Some(text::songs(cx.lib.tracks_by_title.len())),
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
        Row::Open(View::RecentlyPlayed) => cx.recent.to_vec(),
        Row::Folder(f) => lib.folder_tracks(f),
        Row::Genre(g) => albums_tracks(lib, &lib.genres[g].albums),
        Row::Decade(d) => albums_tracks(lib, in_decade(lib, d)),
        Row::Open(View::RecentlyAdded) => albums_tracks(lib, &lib.albums_by_added),
        Row::PlayAll | Row::Shuffle => match view {
            View::Folder(f) => lib.folder_tracks(f),
            View::Genre(g) => albums_tracks(lib, &lib.genres[g].albums),
            View::Decade(d) => albums_tracks(lib, in_decade(lib, d)),
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
        View::SongMenu(t) | View::SongInfo(t) => Some(t),
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
        View::Genre(g) => Some(albums(&lib.genres[g].albums)),
        View::Decade(d) => Some(albums(in_decade(lib, d))),
        _ => None,
    }
}

pub(crate) fn name(lib: &Library, row: Row) -> Option<&str> {
    match row {
        Row::Artist(a) => Some(&lib.artists[a].name),
        Row::Album(a) => Some(&lib.albums[a].name),
        Row::Track(t) => Some(&lib.tracks[t].title),
        Row::Author(a) => Some(&lib.artists[a].name),
        Row::Genre(g) => Some(&lib.genres[g].name),
        Row::Folder(f) => Some(&lib.folders[f].name),
        Row::Playlist(p) => Some(&lib.playlists[p].name),
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
    if forward {
        (at + 1..rows.len())
            .find(|&i| group(i) != here)
            .unwrap_or(at)
    } else {
        match start(at) {
            0 => 0,
            first => start(first - 1),
        }
    }
}

/// The decade album `a` came out in: 1994 is the 1990s.
fn decade(lib: &Library, a: AlbumId) -> u32 {
    lib.albums[a].year / 10 * 10
}

/// The albums of decade `d`, by year: a run of the library's albums by year.
fn in_decade(lib: &Library, d: u32) -> &[AlbumId] {
    let by_year = &lib.albums_by_year;
    let start = by_year.partition_point(|&a| decade(lib, a) < d);
    let end = by_year.partition_point(|&a| decade(lib, a) <= d);
    &by_year[start..end]
}
