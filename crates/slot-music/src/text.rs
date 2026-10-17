//! Every word the music screens show, in one place: the English table, to swap for another
//! language. Names from the library, counts and times are data and pass through as they are.

use crate::browse::View;
use slot_player::Leveling;

use crate::fade::Sleep;
use crate::player::Repeat;
use crate::rows::{Info, Row};
use crate::screens::{Legend, MainHold, Target};
use crate::settings::{ListSize, Setting, Settings, Theme};

pub const MUSIC: &str = "Music";
pub const NOW_PLAYING: &str = "Now Playing";
pub const QUEUE: &str = UP_NEXT;
pub const RESUME: &str = "Resume";
pub const SHUFFLE_LIBRARY: &str = "Shuffle Library";
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
pub const UNKNOWN_ALBUM: &str = "Unknown Album";
pub const NO_MUSIC: &str = "No music in the Music folder";
pub const QUEUE_EMPTY: &str = "Nothing up next";
pub const NO_PLAYER: &str = "Player not running";
pub const NOTHING_PLAYING: &str = "Nothing playing";
pub const FAVORITES: &str = "Favorites";
pub const UP_NEXT: &str = "Up Next";
pub const FAVORITE: &str = "Add to Favorites";
pub const UNFAVORITE: &str = "Remove from Favorites";
pub const BOOKS: &str = "Audiobooks";
pub const FINISHED: &str = "Finished";
pub const CHAPTERS: &str = "Chapters";
pub const BOOKMARKS: &str = "Bookmarks";
pub const NO_BOOKMARKS: &str = "No bookmarks: Y marks the moment";

/// A screen's title, or `None` where the library names it (an artist, an album).
pub fn title(view: View) -> Option<&'static str> {
    Some(match view {
        View::Main => MUSIC,
        View::Queue => QUEUE,
        View::Settings => "Music Settings",
        View::Artists => "Artists",
        View::Albums => "Albums",
        View::Tracks => "Songs",
        View::Books => BOOKS,
        View::Chapters => CHAPTERS,
        View::Bookmarks(_) => BOOKMARKS,
        View::Playlists => "Playlists",
        View::RecentlyAdded => "Recently Added",
        View::PlayingMenu | View::RowMenu(_) => NOW_PLAYING,
        View::SongInfo(_) => "Song Info",
        View::NowPlaying => NOW_PLAYING,
        View::Artist(_) | View::Album(_) | View::Playlist(_) => return None,
    })
}

/// A row's name, or `None` where the library names it (an artist, album or track).
pub fn row(row: Row, view: View) -> Option<&'static str> {
    Some(match row {
        Row::NowPlaying => NOW_PLAYING,
        Row::Artists => "Artists",
        Row::Albums => "Albums",
        Row::Tracks => "Songs",
        Row::Playlists => "Playlists",
        Row::RecentlyAdded => "Recently Added",
        Row::Settings => "Settings",
        Row::Rescan => "Update Library",
        Row::Rebuild => "Rebuild Library",
        Row::Books => BOOKS,
        Row::StartOver => "Start Over",
        Row::Chapters => CHAPTERS,
        Row::Bookmarks => BOOKMARKS,
        Row::Speed => "Speed",
        Row::GoToAlbum => "Go to Album",
        Row::GoToArtist => "Go to Artist",
        Row::SongInfo => "Song Info",
        Row::Setting(s) => match s {
            Setting::ScreenOff => "Screen Off",
            Setting::Jump => "Jump Length",
            Setting::Mono => "Mono Audio",
            Setting::Gapless => "Gapless",
            Setting::Crossfade => "Crossfade",
            Setting::SkipSilence => "Skip Silence",
            Setting::Boost => "Volume Boost",
            Setting::Leveling => "Volume Leveling",
            Setting::Repeat => "Repeat",
            Setting::ListSize => "List Size",
            Setting::SleepTimer => "Sleep Timer",
            Setting::Color => "Dynamic Color",
            Setting::Theme => "Theme",
        },
        Row::PlayAll => "Play All",
        Row::Shuffle => match view {
            View::Artists => "Shuffle Artists",
            View::Albums | View::Artist(_) | View::RecentlyAdded => "Shuffle Albums",
            _ => "Shuffle Songs",
        },
        // Named by the library, or by whether the song playing is a favorite.
        Row::Artist(_)
        | Row::Album(_)
        | Row::Track(_)
        | Row::Queued(_)
        | Row::Info(_)
        | Row::Playlist(_)
        | Row::Book(_)
        | Row::Chapter(_)
        | Row::Bookmark(_)
        | Row::Back
        | Row::Finished
        | Row::Favorite => return None,
    })
}

/// What a Song Info row tells.
pub fn info(info: Info) -> &'static str {
    match info {
        Info::Title => "Title",
        Info::Artist => "Artist",
        Info::Album => "Album",
        Info::Number => "Track",
        Info::Format => "Format",
        Info::File => "File",
    }
}

/// A setting's choice; `sleep` is what the sleep timer waits for.
pub fn setting(setting: Setting, settings: &Settings, sleep: Option<Sleep>) -> String {
    let pick = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_string();
    let on_off = |on: bool| pick(on, "On", "Off");
    match setting {
        Setting::ScreenOff => pick(settings.play_screen_off, "Keep Playing", "Pause"),
        Setting::ListSize => match settings.list_size {
            ListSize::Small => "Small",
            ListSize::Normal => "Normal",
            ListSize::Large => "Large",
        }
        .into(),
        Setting::SleepTimer => match sleep {
            None => on_off(false),
            Some(Sleep::Minutes(m)) => format!("{m} min"),
            Some(Sleep::Chapter) => "End of Chapter".into(),
        },
        Setting::Color => on_off(settings.color),
        Setting::Mono => on_off(settings.mono),
        Setting::Gapless => on_off(settings.gapless),
        Setting::SkipSilence => on_off(settings.skip_silence),
        Setting::Crossfade => match settings.crossfade {
            0 => "Off".into(),
            s => format!("{s} s"),
        },
        Setting::Boost => match settings.boost {
            0 => "Off".into(),
            db => format!("+{db} dB"),
        },
        Setting::Leveling => match settings.leveling {
            Leveling::Off => "Off",
            Leveling::Track => "Song",
            Leveling::Album => "Album",
        }
        .into(),
        Setting::Jump => format!("{} s", settings.jump),
        Setting::Repeat => match settings.repeat {
            Repeat::Off => "Off",
            Repeat::All => "All",
            Repeat::One => "One",
        }
        .into(),
        Setting::Theme => match settings.theme {
            Theme::Slot => "Slot",
            Theme::Dark => "Dark",
            Theme::Light => "Light",
        }
        .into(),
    }
}

/// What a short notice in the corner says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    Added(usize),
    Next(usize),
    /// Muse could not play a song.
    Unplayable,
    /// A key was let go on a row where it acts only when held.
    Hold(&'static str),
    /// START held paused or played, away from Now Playing.
    Paused,
    Playing,
    /// The speed X changed to, in hundredths.
    Speed(u16),
    /// Y marked the moment in a book.
    Bookmarked,
}

/// Short, as the corner is: where songs went, and how many.
pub fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Added(n) => format!("{QUEUE} +{n}"),
        Notice::Next(n) => format!("Play Next +{n}"),
        Notice::Unplayable => "Can't play".into(),
        Notice::Hold(key) => format!("Hold {key}"),
        Notice::Paused => "Paused".into(),
        Notice::Playing => "Playing".into(),
        Notice::Bookmarked => "Bookmarked".into(),
        Notice::Speed(x) => format!("Speed {}", speed(f32::from(*x) / 100.0)),
    }
}

/// Under a book's picture: its speed, how far through it is, and the time it has left at
/// that speed.
pub fn book_left(x: f32, done: Option<f64>, left: f64) -> String {
    match done {
        Some(done) => format!("{} · {} · {} left", speed(x), percent(done), clock(left)),
        None => format!("{} · {} left", speed(x), clock(left)),
    }
}

/// The track after this one, under Now Playing.
pub fn up_next(title: &str, artist: &str) -> String {
    format!("{UP_NEXT}: {title} · {artist}")
}

/// A book's menu row that marks it finished, or not.
pub fn finished(finished: bool) -> &'static str {
    match finished {
        true => "Mark as Unfinished",
        false => "Mark as Finished",
    }
}

/// Seconds as players write them: 3:07, or 1:02:07 past the hour.
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s / 3600 {
        0 => format!("{}:{:02}", s / 60, s % 60),
        h => format!("{h}:{:02}:{:02}", s / 60 % 60, s % 60),
    }
}

/// The time into something and its length: 1:07 / 3:20.
pub fn elapsed(at: f64, len: f64) -> String {
    format!("{} / {}", clock(at), clock(len))
}

/// The book's menu row back to where it was before it last moved.
pub fn back_to(pos: f64) -> String {
    format!("Back to {}", clock(pos))
}

/// A place among others: 3/12.
pub fn of(n: usize, of: usize) -> String {
    format!("{n}/{of}")
}

/// How far through, 0 to 1, as a share: 42%.
pub fn percent(done: f64) -> String {
    format!("{:.0}%", done * 100.0)
}

/// A speed as players write it: 1×, 1.25×.
pub fn speed(x: f32) -> String {
    let s = format!("{x:.2}");
    format!("{}×", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Before the album's or artist's name on a menu row that goes there, set dim.
pub const GO_TO: &str = "Go to";

/// The library being read, and how far, once the songs are counted.
pub fn reading(done: usize, of: usize) -> String {
    match of {
        0 => "Reading the library".into(),
        of => format!("Reading the library {done}/{of}"),
    }
}

/// Short, for the corner: the library being read, or checked behind one already shown.
pub fn scanning(shown: bool, done: usize, of: usize) -> String {
    let verb = if shown { "Checking" } else { "Reading" };
    match of {
        0 => verb.into(),
        of => format!("{verb} {done}/{of}"),
    }
}

pub fn optimizing(done: usize, of: usize) -> String {
    format!("Optimizing {done}/{of}")
}

/// Marks the hint drawn as Left and Right arrows rather than a button name.
pub const ARROWS: &str = "LEFT RIGHT";

/// A button hint: the button, what a tap does, and what holding it does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Hint {
    pub key: &'static str,
    pub tap: &'static str,
    pub held: Option<&'static str>,
}

const fn tap(key: &'static str, tap: &'static str) -> Hint {
    Hint {
        key,
        tap,
        held: None,
    }
}

const fn hold(key: &'static str, tap: &'static str, held: &'static str) -> Hint {
    Hint {
        key,
        tap,
        held: Some(held),
    }
}

impl Hint {
    /// What it says: the tap, and the hold in brackets, "Play (Next)".
    pub fn label(&self) -> String {
        match self.held {
            Some(held) => format!("{} ({held})", self.tap),
            None => self.tap.into(),
        }
    }
}

/// A screen's button hints.
pub fn legend(legend: Legend) -> Vec<Hint> {
    let pick = |on: bool, yes, no| if on { yes } else { no };
    // Labels hold still as the state changes, which the symbols show: a heart, shuffle.
    match legend {
        // SELECT goes to what plays, so it shows only when something does. A held on a list
        // shuffles it all; on Audiobooks, plays on the latest book. X held on Now Playing stops
        // it all, so a tap only says to hold it.
        Legend::Main {
            playing,
            hold: held,
            stop,
        } => {
            let open = match held {
                Some(MainHold::Shuffle) => hold("A", "Open", "Shuffle"),
                Some(MainHold::Resume) => hold("A", "Open", "Resume"),
                None => tap("A", "Open"),
            };
            let mut hints = vec![tap("B", "Back"), open];
            hints.extend(stop.then_some(tap("X", "Hold to Stop")));
            hints.extend(playing.then_some(tap("SELECT", "Playing")));
            hints
        }
        // B and SELECT go unsaid (the Music menu shows SELECT). A also puts the row's songs
        // next when held, X adds them to the queue, and Y puts a song in Favorites or takes it out.
        // A song's START menu goes unsaid: it only goes to its album or artist, or its info.
        Legend::List(Target::Book) => {
            vec![tap("A", "Play"), tap("Y", BOOKMARKS), tap("START", "Menu")]
        }
        Legend::List(target) => {
            let open = match target {
                Target::Song | Target::All => "Play",
                _ => "Open",
            };
            let mut hints = vec![hold("A", open, "Next"), tap("X", "Add to Queue")];
            hints.extend((target == Target::Song).then(|| tap("Y", "Favorite")));
            hints
        }
        // A held puts the track next, but the one playing is already there.
        Legend::Queue { restart } => vec![
            match restart {
                true => tap("A", "Restart"),
                false => hold("A", "Play", "Next"),
            },
            tap("X", "Remove"),
            hold("Y", "Move", "Favorite"),
        ],
        Legend::Carry => vec![tap("B", "Cancel"), tap("Y", "Drop"), tap("R1", "Play Next")],
        Legend::NowPlaying { playing } => vec![
            tap("A", pick(playing, "Pause", "Play")),
            tap("X", "Shuffle"),
            tap("Y", "Favorite"),
            tap("SELECT", "Up Next"),
        ],
        // Left and Right jump, as the arrows say; X steps the speed, which START's menu sets
        // too; Y marks the moment, or held lists the marks. SELECT's chapters are on that menu as
        // well.
        Legend::Book { playing, jump } => vec![
            tap("A", pick(playing, "Pause", "Play")),
            tap(
                ARROWS,
                match jump {
                    10 => "10 s",
                    15 => "15 s",
                    _ => "30 s",
                },
            ),
            tap("X", "Speed"),
            hold("Y", "Bookmark", "List"),
        ],
        Legend::Chapters => vec![tap("B", "Back"), tap("A", "Play")],
        Legend::Bookmarks => vec![tap("B", "Back"), tap("A", "Play"), tap("X", "Remove")],
        Legend::Settings => vec![tap("B", "Back"), tap(ARROWS, "Change")],
        Legend::Menu => vec![tap("B", "Back"), tap("A", "Open"), tap(ARROWS, "Change")],
        Legend::Hold => vec![tap("B", "Back"), tap("A", "Hold to Rebuild")],
        Legend::Back => vec![tap("B", "Back")],
    }
}
