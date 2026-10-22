//! What the music screens show, one file per screen, each with what its keys do.

pub(crate) mod binds;
pub(crate) mod book;
pub(crate) mod dark;
pub(crate) mod equalizer;
pub(crate) mod help;
pub(crate) mod list;
pub(crate) mod lyrics;
pub(crate) mod main;
pub(crate) mod menu;
pub(crate) mod now_playing;
pub(crate) mod queue;
pub(crate) mod settings;

use std::time::Duration;

use crate::browse::rows::RowText;
use crate::browse::rows::{self, Row};
use crate::browse::{Browser, Context, View};
use crate::playback::player::Repeat;

use crate::text::Notice;
use crate::{text, Art, Music};

/// How long a notice like "+N" shows in the corner.
const NOTICE: Duration = Duration::from_millis(1500);
/// How long the letter a list moved to shows.
pub(crate) const LETTER: Duration = Duration::from_millis(600);
/// How far into a hold its ring shows, so a tap does not flash it.
const RING_AFTER: f32 = 0.25;

/// Which button hints a screen shows, with the state they word.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Legend {
    /// The main menu: whether something plays for SELECT to go to, and what A held does on the
    /// row.
    Main {
        playing: bool,
        hold: Option<MainHold>,
    },
    /// A library list, by what its highlighted row is.
    List(Target),
    /// The queue: A starts the highlighted track over when it plays.
    Queue {
        restart: bool,
    },
    /// The queue while a track is carried.
    Carry,
    /// Now Playing: A pauses or plays, and X shows the lyrics when the song has any.
    NowPlaying {
        playing: bool,
        lyrics: bool,
    },
    /// The lyrics: A pauses or plays, or plays from the line scrolled to; X closes them.
    Lyrics {
        playing: bool,
        line: bool,
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
    /// The equalizer's presets: A chooses one, and X held deletes one's `own`.
    Equalizer {
        own: bool,
    },
    /// New Preset's bands: moved, and kept.
    Tune,
    /// A START menu, which may hold a setting.
    Menu,
    /// Update Library: what changed, or held, everything again.
    Update,
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
    /// An audiobook: A plays it on, X lists its chapters, Y its bookmarks, START has its menu.
    Book,
    /// A book's author: A lists their books.
    Author,
}

impl Legend {
    /// Every legend, for checks on how they fit.
    pub fn all() -> Vec<Legend> {
        let on = [false, true];
        let mut all = vec![Legend::Carry, Legend::Settings, Legend::Menu, Legend::Tune];
        all.extend([false, true].map(|own| Legend::Equalizer { own }));
        all.extend([
            Legend::Update,
            Legend::Back,
            Legend::Chapters,
            Legend::Bookmarks,
        ]);
        for hold in [None, Some(MainHold::Shuffle), Some(MainHold::Resume)] {
            all.extend(on.map(|playing| Legend::Main { playing, hold }));
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
                Target::Author,
            ]
            .map(Legend::List),
        );
        all.extend(on.map(|restart| Legend::Queue { restart }));
        for (playing, more) in on.into_iter().flat_map(|a| on.map(|b| (a, b))) {
            all.push(Legend::NowPlaying {
                playing,
                lyrics: more,
            });
            all.push(Legend::Lyrics {
                playing,
                line: more,
            });
        }
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
    /// The letter a list just jumped to, shown large in the middle.
    pub letter: Option<char>,
    /// A key held to act, filling a ring until it does.
    pub hold: Option<Hold>,
}

/// A key held to act: how far, 0 to 1, and which of the legend's hints is its, for the ring to
/// fill around its key; none where it has no hint, and the ring fills in the corner.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Hold {
    pub done: f32,
    pub hint: Option<usize>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Body {
    List(ListBody),
    NowPlaying(NowPlayingBody),
    /// A line in place of a list: the library being read, nothing playing.
    Message(String),
    /// Paragraphs to read, one held in the middle: lyrics.
    Text(TextBody),
}

/// Lines of text, centred, `focus` in the middle. `lit` is the one lit, the rest dim, when the
/// lines follow along (`Some`, if none is yet); else every line is lit.
#[derive(Clone, PartialEq, Debug)]
pub struct TextBody {
    pub lines: Vec<String>,
    pub lit: Option<Option<usize>>,
    pub focus: usize,
    /// How far into the song, 0 to 1, and its time, as Now Playing says it.
    pub progress: Option<f32>,
    pub time: Option<String>,
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
    /// Its title is shown above it: every list and menu but the Music menu, bare as slot's own.
    pub titled: bool,
    /// Its rows are two lines, the name over who made it (`after`); and start with their album's
    /// thumbnail, or room for it until it is read.
    pub two_lines: bool,
    /// How many action rows (Play All) lead its first page, whose room two-line rows keep on
    /// every page so they look alike on each.
    pub actions: usize,
    /// What plays is paused, and its mark says so.
    pub paused: bool,
    pub thumbs: bool,
    /// Its rows are books: a thumbnail not read, or with no picture, shows a book, not a note.
    pub books: bool,
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
    /// A book's: with no picture, the square shows a book rather than a note.
    pub book: bool,
    /// A book's speed, worded, when it is not as recorded: shown beside the state symbols, as
    /// repeat and shuffle are for music.
    pub speed: Option<String>,
    /// With the visualizer on, the bars behind it all, from the bass up, each 0 to 31.
    pub bars: Option<slot_player::Bands>,
}

impl NowPlayingBody {
    /// The bars' heights, each 0 to 1 of the tallest; none with the visualizer off.
    pub fn bar_heights(&self) -> Vec<f32> {
        let top = f32::from(crate::music::BAR_STEPS - 1);
        let bars = self.bars.iter().flatten();
        bars.map(|&b| f32::from(b) / top).collect()
    }
}

/// Whether music plays, how it repeats, whether the queue is shuffled, and whether the song is
/// a favorite.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Status {
    pub playing: bool,
    pub repeat: Repeat,
    pub shuffled: bool,
    /// The song is in Favorites.
    pub favorite: bool,
}

impl Screen {
    fn new(title: impl Into<String>, legend: Legend, body: Body) -> Screen {
        Screen {
            title: title.into(),
            corner: String::new(),
            legend,
            body,
            depth: 0,
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
        let began = std::time::Instant::now();
        let screen = self.build_screen();
        self.note_screen(began.elapsed());
        screen
    }

    fn build_screen(&self) -> Screen {
        let screen = match self.browser.view() {
            View::Main => main::screen(self),
            View::Settings
            | View::MusicSettings
            | View::BookSettings
            | View::Advanced
            | View::Experimental
            | View::Equalizer
            | View::NewPreset => list::rows_screen(self),
            View::Queue => queue::screen(self),
            View::Lyrics => lyrics::screen(self),
            View::Help => help::screen(self),
            View::NowPlaying => now_playing::screen(self),
            View::PlayingMenu
            | View::QueueMenu
            | View::SongMenu(_)
            | View::BookMenu(_)
            | View::SongInfo(_) => menu::screen(self),
            View::Library
            | View::Artists
            | View::Artist(_)
            | View::Albums
            | View::Album(_)
            | View::Tracks
            | View::Genres
            | View::Genre(_)
            | View::Years
            | View::Decade(_)
            | View::Folder(_)
            | View::Playlists
            | View::Playlist(_)
            | View::RecentlyAdded
            | View::RecentlyPlayed
            | View::BookList(_)
            | View::Authors
            | View::Author(_)
            | View::Books => list::screen(self),
            View::Bookmarks(_) | View::BookChapters(_) => book::list(self),
        };
        let now = self.now.at;
        // The corner says, first, a notice; then that the keys are locked, until unlocked; then,
        // on Now Playing, the sleep timer counting down; and else what the screen has there, as
        // a list its count.
        let sleeping = self
            .transport
            .sleep
            .left(now)
            .filter(|_| self.browser.view() == View::NowPlaying)
            .map(|_| text::sleeping(self.transport.sleep.remaining(now)));
        let locked = self
            .buttons
            .pocket
            .locked
            .then(|| text::notice(&Notice::Locked(true)));
        let corner = self
            .notice()
            .or(locked)
            .or(sleeping)
            .unwrap_or(screen.corner);
        Screen {
            depth: self.browser.depth(),
            letter: self
                .flash
                .letter
                .filter(|(_, at)| now.saturating_duration_since(*at) < LETTER)
                .map(|(c, _)| c),
            hold: self
                .buttons
                .keys
                .holding(now)
                .filter(|&(_, done)| done >= RING_AFTER)
                .map(|(key, done)| Hold {
                    done: (done - RING_AFTER) / (1.0 - RING_AFTER),
                    hint: text::hint_of(screen.legend, key),
                }),
            corner,
            ..screen
        }
    }

    /// The thumbnails of the albums the shown page's rows start with, as read.
    pub fn thumbs(&self) -> &[Art] {
        &self.pictures.thumbs
    }

    /// The playing album's picture, on Now Playing only.
    pub fn art(&self) -> Option<&Art> {
        let shown = self.browser.view() == View::NowPlaying && self.player.current().is_some();
        self.pictures.art.as_ref().filter(|_| shown)
    }

    /// A fresh notice, worded.
    fn notice(&self) -> Option<String> {
        let (notice, at) = self.flash.notice.as_ref()?;
        (self.now.at.saturating_duration_since(*at) < NOTICE).then(|| text::notice(notice))
    }
}

/// The legend of the screen `b` shows, from what it is on and how things stand: the one place it
/// is chosen, so the hints the screen shows and the way its keys act are the one table's.
pub(crate) fn legend(b: &Browser, cx: &Context) -> Legend {
    let row = b.row(cx);
    let playing = cx.queue.playing;
    let something = !rows::idle(cx.queue);
    if b.carry.is_some() {
        return Legend::Carry;
    }
    match b.view() {
        View::NowPlaying if cx.book.is_some() => Legend::Book {
            playing,
            jump: cx.settings.jump,
        },
        View::NowPlaying => Legend::NowPlaying {
            playing,
            lyrics: cx.lyrics.lines > 0,
        },
        View::Lyrics => Legend::Lyrics {
            playing,
            line: cx.lyrics.timed && !lyrics::following(b, cx.now),
        },
        View::Main | View::Library => Legend::Main {
            playing: something,
            hold: row.and_then(|row| main::hold(cx, row)),
        },
        View::Books => Legend::Main {
            playing: something,
            hold: None,
        },
        View::Queue => match row {
            Some(Row::Queued(i)) => Legend::Queue {
                restart: i == cx.queue.at,
            },
            _ => Legend::Back,
        },
        // Update Library updates, or held rebuilds; a page opens, with nothing to change.
        View::Settings
        | View::MusicSettings
        | View::BookSettings
        | View::Advanced
        | View::Experimental => match row {
            Some(Row::Rescan) => Legend::Update,
            Some(Row::Open(_)) => Legend::Main {
                playing: something,
                hold: None,
            },
            _ => Legend::Settings,
        },
        // A preset is chosen or deleted; New Preset opens.
        View::Equalizer => match row {
            Some(Row::Preset(i)) => Legend::Equalizer {
                own: cx.equalizer.get(i).is_some_and(|p| p.own),
            },
            _ => Legend::Main {
                playing: something,
                hold: None,
            },
        },
        View::NewPreset => Legend::Tune,
        View::SongInfo(_) | View::Help => Legend::Back,
        // A START menu's row that changes (the sleep timer, repeat, shuffle, speed) says so; the
        // rest open or act, with nothing to change.
        View::PlayingMenu | View::QueueMenu | View::SongMenu(_) | View::BookMenu(_) => match row {
            Some(Row::Setting(_) | Row::ShuffleQueue | Row::Speed) => Legend::Menu,
            _ => Legend::Main {
                playing: something,
                hold: None,
            },
        },
        View::BookChapters(_) => Legend::Chapters,
        View::Bookmarks(_) => Legend::Bookmarks,
        View::Artists
        | View::Artist(_)
        | View::Albums
        | View::Album(_)
        | View::Tracks
        | View::Genres
        | View::Genre(_)
        | View::Years
        | View::Decade(_)
        | View::Folder(_)
        | View::Playlists
        | View::Playlist(_)
        | View::RecentlyAdded
        | View::RecentlyPlayed
        | View::BookList(_)
        | View::Authors
        | View::Author(_) => Legend::List(match row {
            Some(Row::Track(_)) => Target::Song,
            Some(Row::Album(_)) => Target::Album,
            Some(Row::Artist(_)) => Target::Artist,
            Some(
                Row::Playlist(_)
                | Row::Open(View::RecentlyAdded)
                | Row::Open(View::RecentlyPlayed)
                | Row::Genre(_)
                | Row::Folder(_)
                | Row::Decade(_),
            ) => Target::Playlist,
            Some(Row::Book(_)) => Target::Book,
            Some(Row::Author(_)) => Target::Author,
            _ => Target::All,
        }),
    }
}

/// How far `done` is through `of`, 0 to 1, in the steps a bar can show: thousandths, finer than
/// any bar is wide, so the screen changes only as often as its bar may.
pub(crate) fn fraction(done: f64, of: f64) -> Option<f32> {
    (of > 0.0).then(|| ((done / of).clamp(0.0, 1.0) * 1000.0).round() as f32 / 1000.0)
}

/// How many seconds until `x`, moving `rate` a second (counting down where it is negative),
/// next changes its whole part, as a time shown in seconds does; none while it stands. Rounded
/// to the nearest, it changes as `x + 0.5` does; rounded up, as `-x` does going the other way.
pub(crate) fn whole_in(x: f64, rate: f64) -> Option<f64> {
    match rate {
        r if r > 0.0 => Some((x.floor() + 1.0 - x) / r),
        r if r < 0.0 => Some((x - x.floor()) / -r),
        _ => None,
    }
}

/// How many seconds until the screen shows something new of itself, a screen that follows the
/// song: its time, its bar, a book's time left, the line sung, the sleep timer counting down.
/// None on any other screen, or while nothing of it moves; the screen is held to it in tests,
/// as never later than it changes.
pub(crate) fn steps_in(music: &Music) -> Option<f64> {
    let view = music.browser.view();
    let song = match (view, music.book()) {
        (View::NowPlaying, Some((book, _))) => book::steps_in(music, book),
        (View::NowPlaying, None) => now_playing::steps_in(music),
        (View::Lyrics, _) => lyrics::steps_in(music),
        (View::Queue, _) => queue::steps_in(music),
        _ => None,
    };
    // The bars, while there are: they move with every frame.
    let bars =
        (view == View::NowPlaying && music.bars().is_some()).then_some(crate::music::FRAME_S);
    // The countdown in Now Playing's corner, in seconds rounded up.
    let left = music
        .transport
        .sleep
        .remaining(music.now.at)
        .filter(|_| view == View::NowPlaying);
    let sleep = left.and_then(|left| whole_in(-left.as_secs_f64(), 1.0));
    [song, sleep, bars].into_iter().flatten().reduce(f64::min)
}
