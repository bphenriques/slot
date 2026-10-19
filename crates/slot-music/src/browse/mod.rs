//! Moving between the music screens: a stack of views, each with its cursor. What a screen's
//! keys do is in its file under `screens`.

pub mod keys;
pub mod rows;

use crate::books::chapters::Chapter;
use crate::books::Shelf;
use crate::browse::keys::Mode;
use crate::library::Library;
use crate::library::{AlbumId, ArtistId, BookId, TrackId};
use crate::music::ReadAhead;
use crate::playback::fade::Sleep;
use crate::screens;
use crate::settings::{Setting, Settings};
use crate::text::{self, Notice};
use rows::{label, rows, value, Row, RowText};
use std::time::Instant;

/// Rows on a menu page (the music main screen, settings), at the quick menu's spacing.
pub const MENU_ROWS: usize = 7;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum View {
    Main,
    /// The queue, from Now Playing with SELECT: opened at what plays, with what played above it,
    /// faint, as Apple Music has it.
    Queue,
    /// The song's lyrics, from Now Playing with X.
    Lyrics,
    Settings,
    /// The sound settings slot's player has, a page of Music Settings; and the audiobooks'.
    Sound,
    BookSettings,
    /// What helps say how the device does: the logs, and the last sound error.
    Debug,
    Artists,
    Artist(ArtistId),
    Albums,
    Album(AlbumId),
    Tracks,
    NowPlaying,
    Playlists,
    Playlist(usize),
    /// The albums last added, newest first.
    RecentlyAdded,
    /// The songs played last, newest first.
    RecentlyPlayed,
    /// The audiobooks' menu: the book to continue, those being listened to, the authors, all of
    /// them and those finished, as audiobook players have them.
    Books,
    BookList(BookList),
    Authors,
    Author(ArtistId),
    /// A book's chapters, each part's own or else the part, and its bookmarks.
    BookChapters(BookId),
    Bookmarks(BookId),
    /// Now Playing's menu, from START.
    PlayingMenu,
    /// Up Next's and the queue's menu, from START: save it, or clear it.
    QueueMenu,
    /// The menu for a list's song, album, artist or playlist, from START.
    RowMenu(Row),
    /// What is known of a song, set as a list.
    SongInfo(TrackId),
    /// Settings' Integrations page: what the player tells programs outside it.
    Integrations,
    /// Settings' Controls, the keys the hints leave out, and Help, a page to read.
    Controls,
    Help,
}

/// The audiobooks a list holds.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BookList {
    /// Begun and not finished, the latest listened to first.
    Listening,
    /// Every one, by title.
    All,
    /// Those finished, the latest first.
    Finished,
}

impl View {
    /// Menus keep slot's quick-menu size; lists follow List Size.
    pub fn is_menu(self) -> bool {
        matches!(
            self,
            View::Main
                | View::Books
                | View::Settings
                | View::Sound
                | View::BookSettings
                | View::Integrations
                | View::Debug
                | View::PlayingMenu
                | View::QueueMenu
                | View::RowMenu(_)
        )
    }

    /// The menus from START, and Song Info from them: they act on what they were opened on.
    pub fn is_start_menu(self) -> bool {
        matches!(
            self,
            View::PlayingMenu | View::QueueMenu | View::RowMenu(_) | View::SongInfo(_)
        )
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    L1,
    R1,
    A,
    B,
    X,
    Y,
    /// START: the menu for what is shown; held, it pauses.
    Start,
    /// SELECT on its own: to Now Playing, and from there to the queue and back; held, the screen
    /// blanks.
    Select,
    /// SELECT with L1 or R1: the previous or next track, from any music screen.
    SelectL1,
    SelectR1,
    /// L2 and R2: on Now Playing, seek back and on.
    L2,
    R2,
    /// The pocket combo, two of the shoulders and triggers together: held, the music's keys
    /// locked or unlocked.
    Pocket,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    None,
    Leave,
    /// Read the library again: what changed, or every song when true.
    Rescan(bool),
    /// Replace the queue with `tracks`, playing from `start`.
    Play {
        tracks: Vec<TrackId>,
        start: usize,
        shuffle: Shuffle,
    },
    /// Play from this place in the queue.
    Jump(usize),
    /// Add to the end of the queue.
    Enqueue(Vec<TrackId>),
    /// Put right after what plays.
    PlayNext(Vec<TrackId>),
    Toggle,
    Seek(i32),
    Skip(i32),
    /// Shuffle the queue around what plays, or put it back in order.
    ShuffleQueue,
    /// Change a setting: Left (-1) and Right (1) stop at the ends, A (0) goes round.
    Change(Setting, i32),
    /// The cursor jumped to this letter.
    Letter(char),
    /// Play an audiobook where it was left, or from its start.
    PlayBook {
        book: BookId,
        over: bool,
    },
    /// Mark a book finished, or not.
    Finish(BookId),
    /// The playing speed, as a setting steps: -1 and 1 stop at the ends, 0 goes round.
    Speed(i32),
    /// Play a book from its `part`'s `chapter`, or the part's start for none.
    BookChapter {
        book: BookId,
        part: usize,
        chapter: Option<usize>,
    },
    /// Mark where the book playing is; play from a mark, or take it out.
    Bookmark,
    GoBookmark {
        book: BookId,
        mark: usize,
    },
    RemoveBookmark {
        book: BookId,
        mark: usize,
    },
    /// Back to where the book was before it last moved.
    Back,
    /// Turn the screen off until a button, which then acts as it would: SELECT held.
    Blank,
    /// Play from a line of the timed lyrics.
    FromLine(usize),
    /// Take everything out of the queue but what plays.
    Clear,
    /// Keep the queue as a new playlist.
    SaveQueue,
    /// Show a notice in the corner.
    Tell(Notice),
    /// Add this song to Favorites, or take it out.
    Favorite(TrackId),
    /// Take `n` tracks out of the queue from place `at`, short of the one playing.
    Remove {
        at: usize,
        n: usize,
    },
    /// Move the track at the first place in the queue to the second.
    Move(usize, usize),
}

/// How Play orders tracks: as listed, shuffled, or as shuffled groups `sizes` long.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shuffle {
    Off,
    Tracks,
    Groups(Vec<usize>),
}

/// The queue as the browser sees it: its tracks, and the place of the one playing.
#[derive(Copy, Clone, Default)]
pub struct QueueView<'a> {
    pub tracks: &'a [TrackId],
    pub at: usize,
    /// Whether it plays now, rather than paused or waiting.
    pub playing: bool,
    /// A saved queue waits for the library to be read, to be put back.
    pub waiting: bool,
    /// How fast it plays.
    pub speed: f32,
}

/// What the screens read: the library, the queue, the settings and the sleep timer.
#[derive(Copy, Clone)]
pub struct Context<'a> {
    pub lib: &'a Library,
    pub queue: QueueView<'a>,
    pub settings: Settings,
    /// Minutes left on the sleep timer.
    pub sleep: Option<Sleep>,
    /// The audiobooks' places and bookmarks, and the book playing.
    pub shelf: &'a Shelf,
    pub book: Option<BookId>,
    /// Where the book was before it last moved, in seconds into its part.
    pub back: Option<f64>,
    pub lyrics: LyricsAt,
    /// Recently Played's songs, newest first.
    pub recent: &'a [TrackId],
    /// Why the sound last failed: the file, or the device.
    pub last_error: Option<&'a str>,
    /// The queue plays shuffled.
    pub shuffled: bool,
    /// Chapters read from books' files, by part.
    pub chapters: &'a ReadAhead<Vec<Chapter>>,
    /// The equalizer's preset, by name; none when off.
    pub eq: Option<&'a str>,
    /// The moment the keys act at: the frame's, never the clock's.
    pub now: Instant,
}

/// The song's lyrics, as their keys need them: how many lines, none without lyrics; which is
/// sung now; and whether they are timed, to play from a line.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct LyricsAt {
    pub lines: usize,
    pub sung: Option<usize>,
    pub timed: bool,
}

impl<'a> Context<'a> {
    /// The library with an empty queue and the default settings, `now`.
    #[cfg(test)]
    pub fn new(lib: &'a Library, now: Instant) -> Context<'a> {
        Context {
            lib,
            queue: QueueView::default(),
            settings: Settings::default(),
            sleep: None,
            shelf: Shelf::none(),
            book: None,
            back: None,
            lyrics: LyricsAt::default(),
            recent: &[],
            last_error: None,
            shuffled: false,
            chapters: &NO_CHAPTERS,
            eq: None,
            now,
        }
    }
}

/// No chapters read, for a test's context.
#[cfg(test)]
static NO_CHAPTERS: ReadAhead<Vec<Chapter>> = ReadAhead::new();

/// A list screen's page: the rows the cursor is among.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Page {
    pub title: String,
    pub rows: Vec<RowText>,
    /// A setting's value, or a count, beside its row.
    pub values: Vec<Option<String>>,
    pub selected: usize,
    pub pages: usize,
    /// The highlighted item's place from 1, and how many items, not counting action rows.
    pub item: Option<usize>,
    pub items: usize,
    /// Rows this page holds.
    pub per_page: usize,
    /// The row the cursor is on.
    pub highlighted: Option<Row>,
    /// Rows on this page with a rule above them: under the action rows, between discs, and
    /// under Recently Added.
    pub rules: Vec<usize>,
}

struct Level {
    view: View,
    at: usize,
}

pub struct Browser {
    stack: Vec<Level>,
    /// A queued track carried to a new place, and the place it was lifted from.
    pub(crate) carry: Option<usize>,
    /// When the lyrics were last scrolled by hand: they follow the song again a while after.
    pub(crate) scrolled: Option<std::time::Instant>,
    /// How many times X has acted held in the queue: the longer, the more it takes out at once.
    pub(crate) run: usize,
}

impl Default for Browser {
    fn default() -> Self {
        Browser {
            stack: vec![Level {
                view: View::Main,
                at: 0,
            }],
            carry: None,
            scrolled: None,
            run: 0,
        }
    }
}

impl Browser {
    pub fn view(&self) -> View {
        self.top().view
    }

    /// How many screens deep the browser is, 1 on the main screen.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// How `key` acts here: at once, or waiting to see whether it is held; and whether it
    /// repeats; as the screen's key table says, the one its hints are read from.
    pub fn mode(&self, cx: &Context, key: Key) -> Mode {
        let legend = screens::legend(self, cx);
        screens::binds::mode(&screens::binds::binds(legend), key)
    }

    /// `key` held, as `mode` says: what it does then, or again as it repeats. A repeat stops at
    /// the ends of a list, where a fresh press goes round.
    pub fn held(&mut self, cx: &Context, key: Key) -> Outcome {
        match key {
            Key::Start => return Outcome::Toggle,
            Key::Select => return Outcome::Blank,
            Key::B => {
                self.home();
                return Outcome::None;
            }
            _ => {}
        }
        match (self.mode(cx, key), self.view(), self.row(cx)) {
            (Mode::Repeat, ..) if self.at_end(cx, key) => Outcome::None,
            (_, _, Some(Row::Queued(i))) if key == Key::X => screens::queue::held(self, cx, i, key),
            (Mode::Repeat, ..) => self.press(cx, key),
            (_, View::NowPlaying, _) if cx.book.is_some() => screens::book::held(self, cx, key),
            (_, View::NowPlaying, _) => screens::now_playing::held(key),
            (_, View::Lyrics, _) => screens::lyrics::held(key),
            (_, _, Some(Row::Queued(i))) => screens::queue::held(self, cx, i, key),
            (_, _, Some(Row::Rebuild)) => Outcome::Rescan(true),
            (_, View::Main, Some(row)) => screens::main::held(self, cx, row, key),
            (_, view, Some(row)) if !view.is_menu() => screens::list::held(cx, view, row, key),
            _ => Outcome::None,
        }
    }

    /// Whether Up or Down would wrap round the list: a held key stops there instead.
    pub fn at_end(&self, cx: &Context, key: Key) -> bool {
        let n = match self.view() {
            View::Lyrics => cx.lyrics.lines,
            View::Help => text::HELP_TEXT.len(),
            view => rows(cx, view).len(),
        };
        match key {
            Key::Up => self.at() == 0,
            Key::Down => self.at() + 1 >= n,
            // X held in the queue takes out the tracks after, up to the last.
            Key::X => self.view() == View::Queue && self.at() + 1 >= n,
            _ => false,
        }
    }

    pub fn press(&mut self, cx: &Context, key: Key) -> Outcome {
        // Carrying a track takes every key until it is put down.
        if let Some(from) = self.carry {
            return screens::queue::carrying(self, cx, from, key);
        }
        match key {
            // START opens the menu for what is shown; held, it pauses.
            Key::Start => {
                screens::menu::toggle(self, cx);
                return Outcome::None;
            }
            Key::SelectL1 => return Outcome::Skip(-1),
            Key::SelectR1 => return Outcome::Skip(1),
            // SELECT goes to Now Playing, and from there to the queue, or a book's chapters, and
            // back.
            Key::Select => {
                match self.view() {
                    View::NowPlaying => match cx.book {
                        Some(book) => screens::book::open_book_chapters(self, cx, book),
                        None => screens::now_playing::open_queue(self, cx),
                    },
                    View::Queue | View::BookChapters(_) if self.under_now_playing() => self.pop(),
                    _ => self.visit(View::NowPlaying),
                }
                return Outcome::None;
            }
            Key::B if self.stack.len() == 1 => return Outcome::Leave,
            Key::B => {
                self.pop();
                return Outcome::None;
            }
            _ => {}
        }
        match (self.view(), cx.book) {
            (View::NowPlaying, Some(_)) => return screens::book::press(cx, key),
            (View::NowPlaying, None) => return screens::now_playing::press(self, cx, key),
            (View::Lyrics, _) => return screens::lyrics::press(self, cx, key),
            (View::Help, _) => return screens::help::press(self, key),
            _ => {}
        }
        let rows = rows(cx, self.view());
        if rows.is_empty() {
            return Outcome::None;
        }
        let at = self.at().min(rows.len() - 1);
        // The START menus, settings and queued tracks take some keys; the rest move through
        // the list.
        let own = match rows[at] {
            row if self.view().is_start_menu() => screens::menu::press(self, cx, row, key),
            Row::Setting(s) => screens::settings::press(s, key),
            Row::Queued(i) => screens::queue::press(self, cx, i, key),
            Row::BookChapter { .. } | Row::Bookmark(_) => {
                screens::book::press_row(self, rows[at], key)
            }
            _ => None,
        };
        own.unwrap_or_else(|| screens::list::press(self, cx, &rows, at, key))
    }

    pub fn page(&self, cx: &Context) -> Page {
        let view = self.view();
        let rows = rows(cx, view);
        let fixed = rows::fixed(view).len();
        let at = self.at().min(rows.len().saturating_sub(1));
        let per = per_page(view, &cx.settings);
        let page = at / per;
        let shown = || rows.iter().skip(page * per).take(per);
        Page {
            title: match view {
                View::Artist(a) => cx.lib.artists[a].name.clone(),
                View::Album(a) => cx.lib.albums[a].name.clone(),
                View::Playlist(0) => text::FAVORITES.into(),
                View::Playlist(p) => cx.lib.playlists[p].name.clone(),
                view => text::title(view).unwrap_or_default().into(),
            },
            rows: shown().map(|&r| label(cx, view, r)).collect(),
            values: shown().map(|&r| value(cx, r)).collect(),
            selected: at % per,
            highlighted: rows.get(at).copied(),
            rules: (1..per)
                .filter(|&i| {
                    let (above, row) = (page * per + i - 1, page * per + i);
                    match (rows.get(above), rows.get(row)) {
                        (Some(_), Some(_)) if row == fixed => true,
                        // The lists pinned first are kept apart from the playlists made.
                        (Some(Row::RecentlyAdded), Some(_)) => true,
                        (Some(&Row::Track(a)), Some(&Row::Track(b))) => {
                            matches!(view, View::Album(_))
                                && cx.lib.tracks[a].disc != cx.lib.tracks[b].disc
                        }
                        _ => false,
                    }
                })
                .collect(),
            pages: rows.len().div_ceil(per).max(1),
            item: at.checked_sub(fixed).map(|i| i + 1),
            items: rows.len() - fixed,
            per_page: per,
        }
    }

    /// The cursor's row on the screen shown.
    pub fn at(&self) -> usize {
        self.top().at
    }

    pub(crate) fn set_at(&mut self, at: usize) {
        self.stack
            .last_mut()
            .expect("the main screen is never popped")
            .at = at;
    }

    /// Opens `view` with the cursor on row `at`.
    pub(crate) fn push(&mut self, view: View, at: usize) {
        self.carry = None;
        self.stack.push(Level { view, at });
    }

    /// The row the cursor is on: the last, where the list has grown shorter under it, as every
    /// read of the cursor has it.
    pub(crate) fn row(&self, cx: &Context) -> Option<Row> {
        let rows = rows(cx, self.view());
        rows.get(self.at().min(rows.len().saturating_sub(1)))
            .copied()
    }

    /// Goes all the way back, to the main screen.
    pub fn home(&mut self) {
        self.carry = None;
        self.stack.truncate(1);
    }

    /// Goes back a screen; the main screen stays.
    pub(crate) fn pop(&mut self) {
        self.carry = None;
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    /// Whether the screen shown was opened from Now Playing.
    pub(crate) fn under_now_playing(&self) -> bool {
        let n = self.stack.len();
        n >= 2 && self.stack[n - 2].view == View::NowPlaying
    }

    /// Opens `view`, or goes back if it is open.
    fn visit(&mut self, view: View) {
        match self.view() {
            v if v == view => self.pop(),
            _ => self.push(view, 0),
        }
    }

    fn top(&self) -> &Level {
        self.stack.last().expect("the main screen is never popped")
    }
}

/// Rows a page of `view` holds: menus keep slot's size, lists follow List Size.
pub fn per_page(view: View, settings: &Settings) -> usize {
    if view.is_menu() {
        MENU_ROWS
    } else {
        settings.list_size.page()
    }
}
