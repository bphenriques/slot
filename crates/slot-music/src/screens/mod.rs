//! What the music screens show, one file per screen, each with what its keys do.

pub(crate) mod book;
pub(crate) mod dark;
pub(crate) mod list;
pub(crate) mod main;
pub(crate) mod menu;
pub(crate) mod now_playing;
pub(crate) mod queue;
pub(crate) mod settings;

use std::time::Duration;

use crate::browse::{Context, Key, View};
use crate::player::Repeat;
use crate::rows::RowText;

use crate::{text, Art, Music};

/// How long a notice like "+N" shows in the corner.
const NOTICE: Duration = Duration::from_millis(1500);
/// How long the letter a list jumped to shows.
const LETTER: Duration = Duration::from_millis(600);
/// How far into a hold its ring shows, so a tap does not flash it.
const RING_AFTER: f32 = 0.25;

/// Which button hints a screen shows, with the state they word.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Legend {
    /// The main menu: whether something plays for SELECT to go to, what A held does on the row,
    /// and whether X held stops it all there.
    Main {
        playing: bool,
        hold: Option<MainHold>,
        stop: bool,
    },
    /// A library list, by what its highlighted row is.
    List(Target),
    /// The queue: A starts the highlighted track over when it plays.
    Queue {
        restart: bool,
    },
    /// The queue while a track is carried.
    Carry,
    /// Now Playing: A pauses or plays.
    NowPlaying {
        playing: bool,
    },
    /// An audiobook playing: A pauses or plays, and the arrows jump `jump` seconds.
    Book {
        playing: bool,
        jump: u32,
    },
    /// A book's chapters, to play from; and its bookmarks, to play from or take out.
    Chapters,
    Bookmarks,
    Settings,
    /// A START menu, which may hold a setting.
    Menu,
    /// Rebuild Library, which acts only when A is held, as it takes long.
    Hold,
    Back,
}

/// What A held does on a main menu row: shuffle the songs a list holds, or play on the latest
/// audiobook.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MainHold {
    Shuffle,
    Resume,
}

/// What a list's highlighted row stands for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Target {
    /// A song: Y puts it in Favorites or takes it out.
    Song,
    Album,
    Artist,
    Playlist,
    /// Play All or Shuffle: the whole list.
    All,
    /// An audiobook: A plays it on, Y lists its bookmarks, START has its menu.
    Book,
}

impl Legend {
    /// Every legend, for checks on how they fit.
    pub fn all() -> Vec<Legend> {
        let on = [false, true];
        let mut all = vec![Legend::Carry, Legend::Settings, Legend::Menu];
        all.extend([
            Legend::Hold,
            Legend::Back,
            Legend::Chapters,
            Legend::Bookmarks,
        ]);
        for hold in [None, Some(MainHold::Shuffle), Some(MainHold::Resume)] {
            for stop in on {
                all.extend(on.map(|playing| Legend::Main {
                    playing,
                    hold,
                    stop,
                }));
            }
        }
        for jump in [10, 15, 30] {
            all.extend(on.map(|playing| Legend::Book { playing, jump }));
        }
        all.extend(
            [
                Target::Song,
                Target::Album,
                Target::Artist,
                Target::Playlist,
                Target::All,
                Target::Book,
            ]
            .map(Legend::List),
        );
        all.extend(on.map(|restart| Legend::Queue { restart }));
        all.extend(on.map(|playing| Legend::NowPlaying { playing }));
        all
    }
}

/// A music screen: a header, a body and the button hints.
#[derive(Clone, PartialEq, Debug)]
pub struct Screen {
    pub title: String,
    /// Small at the top right: a count, a notice or the album pictures being made.
    pub corner: String,
    pub legend: Legend,
    pub body: Body,
    /// How deep the screen is, so a change slides the way it went.
    pub depth: usize,
    /// Whether the header shows slot's clock and battery (`clock = on`).
    pub clock: bool,
    /// The letter a list just jumped to, shown large in the middle.
    pub letter: Option<char>,
    /// A key held to act, and how far, 0 to 1: a ring fills until it does, and the key's hint
    /// says what the hold does.
    pub hold: Option<(Key, f32)>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Body {
    List(ListBody),
    NowPlaying(NowPlayingBody),
    /// A line in place of a list: the library being read, nothing playing.
    Message(String),
}

/// A page of rows with one highlighted.
#[derive(Clone, PartialEq, Debug)]
pub struct ListBody {
    pub rows: Vec<RowText>,
    /// A setting's value or a count beside its row.
    pub values: Vec<Option<String>>,
    pub selected: usize,
    /// Left and Right change the values: arrows show by the highlighted one.
    pub adjustable: bool,
    /// Menus keep slot's quick-menu size; lists follow List Size.
    pub menu: bool,
    /// Rows a page holds; a menu on one page, the rows it has.
    pub per_page: usize,
    /// Rows with a rule above them: under the action rows, like Play All, and between discs.
    pub rules: Vec<usize>,
    /// The album pictures being made, 0 to 1, as a thin bar under the title.
    pub progress: Option<f32>,
    /// The highlighted row is lifted, carried to a new place.
    pub lifted: bool,
    /// Rows that only tell, like Song Info's: no highlight, and every long one scrolls.
    pub readonly: bool,
}

/// The track playing. Room is kept for its album picture whether or not it has loaded.
#[derive(Clone, PartialEq, Debug)]
pub struct NowPlayingBody {
    /// The song, or a book's chapter; who it is by; and what it is from, its album or book.
    pub title: String,
    pub by: String,
    pub from: String,
    pub status: Status,
    /// How far into the track, 0 to 1, once its length is known.
    pub progress: Option<f32>,
    /// The track's place in the queue, and the time into it and its length.
    pub place: String,
    pub time: String,
    /// The track after this one, when Up Next is on.
    pub next: Option<String>,
    /// Where bookmarks are along the bar, 0 to 1: a book's, in its chapter.
    pub marks: Vec<f32>,
    /// Whether the screen takes its colour from the album picture.
    pub color: bool,
}

/// Whether music plays, how it repeats, whether the queue is shuffled, and whether the song is
/// a favorite.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Status {
    pub playing: bool,
    pub repeat: Repeat,
    pub shuffled: bool,
    /// The song is in Favorites, and the sleep timer counts down.
    pub favorite: bool,
    pub sleeping: bool,
}

impl Screen {
    fn new(title: impl Into<String>, legend: Legend, body: Body) -> Screen {
        Screen {
            title: title.into(),
            corner: String::new(),
            legend,
            body,
            depth: 0,
            clock: false,
            letter: None,
            hold: None,
        }
    }

    fn message(title: &str, line: &str) -> Screen {
        Screen::new(title, Legend::Back, Body::Message(line.into()))
    }
}

impl Music {
    pub fn screen(&self) -> Screen {
        let screen = match self.browser.view() {
            View::Main => main::screen(self),
            View::Settings => settings::screen(self),
            View::Queue => queue::screen(self),
            View::NowPlaying => now_playing::screen(self),
            View::PlayingMenu | View::RowMenu(_) | View::SongInfo(_) => menu::screen(self),
            View::Artists
            | View::Artist(_)
            | View::Albums
            | View::Album(_)
            | View::Tracks
            | View::Playlists
            | View::Playlist(_)
            | View::RecentlyAdded
            | View::Books => list::screen(self),
            View::Chapters | View::Bookmarks(_) => book::list(self),
        };
        // A notice shows in any corner left empty, Now Playing's too.
        let corner = match screen.corner.is_empty() {
            true => self.notice().unwrap_or_default(),
            false => screen.corner,
        };
        Screen {
            depth: self.browser.depth(),
            clock: self.settings.clock,
            letter: self
                .letter
                .filter(|(_, at)| at.elapsed() < LETTER)
                .map(|(c, _)| c),
            hold: self
                .keys
                .holding()
                .filter(|&(_, done)| done >= RING_AFTER)
                .map(|(key, done)| (key, (done - RING_AFTER) / (1.0 - RING_AFTER))),
            corner,
            ..screen
        }
    }

    /// The playing album's picture, on Now Playing only.
    pub fn art(&self) -> Option<&Art> {
        let shown = self.browser.view() == View::NowPlaying && self.player.current().is_some();
        self.art.as_ref().filter(|_| shown)
    }

    /// Songs the scan has gone through, of those it found.
    pub(crate) fn reading(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering::Relaxed;
        (
            self.reading.done.load(Relaxed),
            self.reading.of.load(Relaxed),
        )
    }

    /// What the screens and their keys read.
    pub(crate) fn context(&self) -> Context<'_> {
        Context {
            lib: &self.library,
            queue: self.player.queue(),
            settings: self.settings,
            sleep: self.sleep.left(),
            shelf: &self.shelf,
            book: self.book().map(|(b, _)| b),
            back: self.back.map(|(_, _, pos)| pos),
        }
    }

    /// A fresh notice, worded.
    fn notice(&self) -> Option<String> {
        let (notice, at) = self.notice.as_ref()?;
        (at.elapsed() < NOTICE).then(|| text::notice(notice))
    }
}
