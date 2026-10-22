//! Every word the music screens show, in one place: the English table, to swap for another
//! language. Names from the library, counts and times are data and pass through as they are.

use std::time::Duration;

use crate::browse::rows::{Info, Row};
use crate::browse::{BookList, Key, View};
use crate::playback::fade::Sleep;
use crate::playback::player::Repeat;
use crate::screens::binds::binds;
use crate::screens::Legend;
use crate::settings::{ScreenOff, Setting, Settings};

pub const MUSIC: &str = "Music";
pub const NOW_PLAYING: &str = "Now Playing";
pub const QUEUE: &str = "Queue";
pub const SAVED_QUEUE: &str = "Saved Queue";
pub const ADVANCED: &str = "Advanced";
pub const EXPERIMENTAL: &str = "Experimental";
pub const HELP: &str = "Help";

/// Help's keys: those the hints leave out, by where they act, each with what it does.
pub const KEYS: [(&str, &[(&str, &str)]); 6] = [
    (
        "Everywhere",
        &[
            ("START", "Menu for what is shown"),
            ("START held", "Play or pause"),
            ("SELECT", "Now Playing, then the queue"),
            ("SELECT + L1 / R1", "Previous / next song"),
            ("B held", "Back to the Music menu"),
            ("MENU", "Back to slot"),
        ],
    ),
    (
        "Lists",
        &[
            ("L1 / R1", "Letter, or page"),
            ("Left / Right", "Previous / next page"),
        ],
    ),
    (
        "Now Playing",
        &[
            ("L1 / R1", "Previous / next song"),
            ("Left / Right held", "Seek"),
            ("L2 / R2", "Back / forward 10 s"),
        ],
    ),
    (
        "Audiobooks",
        &[
            ("L1 / R1", "Previous / next chapter"),
            ("SELECT", "Chapters"),
            ("Y held", "Bookmarks"),
        ],
    ),
    ("Pocket", &[("Two shoulders held", "Lock or unlock")]),
    (
        "Screen off",
        &[
            ("L1 / R1", "Skip, or jump in a book"),
            ("L1 / R1 held", "Seek"),
            ("L2 / R2", "Play or pause"),
            ("L2 / R2 held", "Bookmark, in a book"),
        ],
    ),
];

/// Help's first lines: what the screens do not say, a line each.
pub const TIPS: [&str; 5] = [
    "Tap, or hold for what is in brackets.",
    "Songs go in Music.",
    "Books go in Audiobooks.",
    "Playlists go in Music/Playlists.",
    "Find this page in Settings, Help.",
];

/// Help's line `i`: a tip; a place's name, as a heading; or one of its keys and what it does.
/// None past the last.
pub fn help(i: usize) -> Option<HelpLine> {
    if let Some(&tip) = TIPS.get(i) {
        return Some(HelpLine::Tip(tip));
    }
    let mut at = i - TIPS.len();
    for (place, keys) in KEYS {
        if at == 0 {
            return Some(HelpLine::Place(place));
        }
        if let Some(&(key, does)) = keys.get(at - 1) {
            return Some(HelpLine::Key(key, does));
        }
        at -= 1 + keys.len();
    }
    None
}

/// How many lines Help has.
pub fn help_len() -> usize {
    TIPS.len() + KEYS.iter().map(|(_, keys)| 1 + keys.len()).sum::<usize>()
}

/// A line of Help.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HelpLine {
    Tip(&'static str),
    Place(&'static str),
    Key(&'static str, &'static str),
}
pub const NONE: &str = "None";
pub const RECENTLY_PLAYED: &str = "Recently Played";
pub const LIBRARY: &str = "Library";
pub const GENRES: &str = "Genres";
pub const YEARS: &str = "Years";
pub const RESUME: &str = "Resume";
pub const SHUFFLE_LIBRARY: &str = "Shuffle Library";
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
pub const UNKNOWN_ALBUM: &str = "Unknown Album";
/// Who an album of songs by several artists, its tags naming none for it, is filed under.
pub const VARIOUS_ARTISTS: &str = "Various Artists";
/// What a library or audiobooks screen says with nothing to list: where to put it, the folder
/// by its name.
pub fn no_music(folder: &str) -> String {
    format!("Add songs to {folder}")
}

pub fn no_books(folder: &str) -> String {
    format!("Add books to {folder}")
}
/// The Music menu's corner on a card with neither.
pub const NOTHING_YET: &str = "No music or books";
pub const QUEUE_EMPTY: &str = "Nothing up next";
pub const NO_PLAYER: &str = "No sound device";
pub const NOTHING_PLAYING: &str = "Nothing playing";
pub const FAVORITES: &str = "Favorites";
pub const UP_NEXT: &str = "Up Next";
pub const LYRICS: &str = "Lyrics";
/// Settings' page of how music plays, beside its Audiobooks page.
pub const MUSIC_SETTINGS: &str = "Music";
pub const NO_LYRICS: &str = "No lyrics for this song";
pub const BOOKS: &str = "Audiobooks";
pub const FINISHED: &str = "Finished";
pub const CHAPTERS: &str = "Chapters";
pub const BOOKMARKS: &str = "Bookmarks";
pub const FOLDERS: &str = "Folders";
pub const NO_BOOKMARKS: &str = "No bookmarks yet";
/// What Recently Played, Favorites and a playlist say with no songs in them yet.
pub const NOTHING_PLAYED: &str = "Nothing played yet";
pub const NO_FAVORITES: &str = "No favorites yet";
pub const NO_SONGS: &str = "No songs yet";
/// The lyrics' corner, when they are timed and follow the song.
pub const SYNCED: &str = "Synced";
pub const EQUALIZER: &str = "Equalizer";
/// The equalizer's row that makes a preset of one's own, and its page.
pub const NEW_PRESET: &str = "New Preset";

/// A screen's title, or `None` where the library names it (an artist, an album).
pub fn title(view: View) -> Option<&'static str> {
    Some(match view {
        View::Main => MUSIC,
        View::Library => LIBRARY,
        View::Genres => GENRES,
        View::Years => YEARS,
        View::Folder(_) => FOLDERS,
        View::Lyrics => LYRICS,
        View::Queue => QUEUE,
        View::Settings => "Settings",
        View::MusicSettings => MUSIC_SETTINGS,
        View::BookSettings => BOOKS,
        View::Advanced => ADVANCED,
        View::Experimental => EXPERIMENTAL,
        View::Equalizer => EQUALIZER,
        View::NewPreset => NEW_PRESET,
        View::Help => HELP,
        View::Artists => "Artists",
        View::Albums => "Albums",
        View::Tracks => "Songs",
        View::Books => BOOKS,
        View::BookList(list) => book_list(list),
        View::Authors => AUTHORS,
        View::BookChapters(_) => CHAPTERS,
        View::Bookmarks(_) => BOOKMARKS,
        View::Playlists => "Playlists",
        View::RecentlyAdded => "Recently Added",
        View::RecentlyPlayed => RECENTLY_PLAYED,
        View::PlayingMenu | View::SongMenu(_) | View::BookMenu(_) => NOW_PLAYING,
        View::QueueMenu => QUEUE,
        View::SongInfo(_) => "Song Info",
        View::NowPlaying => NOW_PLAYING,
        View::Artist(_)
        | View::Album(_)
        | View::Playlist(_)
        | View::Author(_)
        | View::Genre(_)
        | View::Decade(_) => return None,
    })
}

/// A row's name, or `None` where the library names it (an artist, album or track).
pub fn row(row: Row, view: View) -> Option<&'static str> {
    Some(match row {
        Row::NowPlaying => NOW_PLAYING,
        // Named as the page it opens; Settings, short of its title.
        Row::Open(View::Settings) => "Settings",
        Row::Open(view) => return title(view),
        Row::Rescan => "Update Library",
        Row::Continue => CONTINUE,
        Row::StartOver => "Start Over",
        Row::Chapters => CHAPTERS,
        Row::Bookmarks => BOOKMARKS,
        Row::Speed => "Speed",
        Row::GoToAlbum => "Go to Album",
        Row::GoToArtist => "Go to Artist",
        Row::SongInfo => "Song Info",
        Row::ShuffleQueue => "Shuffle",
        Row::SaveQueue => "Save as Playlist",
        Row::ClearQueue => "Clear Queue",
        Row::NewPreset => NEW_PRESET,
        Row::Setting(s) => match s {
            Setting::ScreenOff => "Screen Off",
            Setting::OpenPlaying => "Open on Now Playing",
            Setting::Jump => "Jump Length",
            Setting::Mono => "Mono Audio",
            Setting::Visualizer => "Visualizer",
            Setting::Crossfade => "Crossfade",
            Setting::SkipSilence => "Skip Silence",
            Setting::PerfLog => "Performance Log",
            Setting::Log => "Detailed Log",
            Setting::Scrobbler => "Scrobbler Log",
            Setting::Boost => "Volume Boost",
            Setting::Leveling => "Volume Leveling",
            Setting::Repeat => "Repeat",
            Setting::SleepTimer => "Sleep Timer",
        },
        Row::PlayAll => "Play All",
        Row::Shuffle => match view {
            View::Artists => "Shuffle Artists",
            View::Albums | View::Artist(_) | View::RecentlyAdded => "Shuffle Albums",
            _ => "Shuffle Songs",
        },
        // Named by the library, or by whether the song playing is a favorite.
        Row::Genre(_)
        | Row::Folder(_)
        | Row::Decade(_)
        | Row::Artist(_)
        | Row::Album(_)
        | Row::Author(_)
        | Row::Track(_)
        | Row::Queued(_)
        | Row::Info(_)
        | Row::HelpLine(_)
        | Row::Playlist(_)
        | Row::Book(_)
        | Row::BookChapter { .. }
        | Row::Bookmark(_)
        | Row::Preset(_)
        | Row::Band(_)
        | Row::Back
        | Row::Finished => return None,
    })
}

/// A preset made on the device, named by how many came before: renamed on a computer.
pub fn own_preset(n: usize) -> String {
    format!("Custom {n}")
}

/// An equalizer band by where it is: "60 Hz", "3.6 kHz".
pub fn hz(hz: f32) -> String {
    if hz >= 1000.0 {
        let k = format!("{:.1}", hz / 1000.0);
        format!("{} kHz", k.trim_end_matches(".0"))
    } else {
        format!("{hz:.0} Hz")
    }
}

/// How far a band goes up or down: "+3 dB", "0 dB", "−3 dB".
pub fn db(db: f32) -> String {
    let n = format!("{:.1}", db.abs());
    let n = n.trim_end_matches(".0");
    match db {
        d if d > 0.0 => format!("+{n} dB"),
        d if d < 0.0 => format!("−{n} dB"),
        _ => "0 dB".into(),
    }
}

/// A choice that is on or off.
pub fn on_off(on: bool) -> &'static str {
    if on {
        "On"
    } else {
        "Off"
    }
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
pub fn setting(setting: Setting, settings: &Settings, sleep: Option<Sleep>, book: bool) -> String {
    let on_off = |on: bool| on_off(on).to_string();
    let pick = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_string();
    match setting {
        Setting::ScreenOff => match settings.screen_off {
            ScreenOff::Pause => "Pause",
            ScreenOff::Play => "Keep Playing",
            ScreenOff::PlayLocked => "Play Locked",
        }
        .into(),
        Setting::OpenPlaying => on_off(settings.open_playing),
        Setting::SleepTimer => match sleep {
            None => on_off(false),
            Some(Sleep::Minutes(m)) => minutes(m),
            Some(Sleep::End) => pick(book, "End of Chapter", "End of Song"),
        },
        Setting::Mono => on_off(settings.mono),
        Setting::Visualizer => on_off(settings.visualizer),
        Setting::SkipSilence => on_off(settings.skip_silence),
        Setting::PerfLog => on_off(settings.perf),
        Setting::Log => on_off(settings.log),
        Setting::Scrobbler => on_off(settings.scrobbler),
        Setting::Crossfade => match settings.crossfade {
            0 => "Off".into(),
            s => format!("{s} s"),
        },
        Setting::Boost => match settings.boost {
            0 => "Off".into(),
            db => format!("+{db} dB"),
        },
        Setting::Leveling => on_off(settings.leveling),
        Setting::Jump => format!("{} s", settings.jump),
        Setting::Repeat => match settings.repeat {
            Repeat::Off => "Off",
            Repeat::All => "All",
            Repeat::One => "One",
        }
        .into(),
    }
}

/// The words before a name that it is not sorted by, as players have them: The Beatles under B.
pub const ARTICLES: &[&str] = &["The"];

/// What a short notice in the corner says.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    Added(usize),
    Next(usize),
    /// The player could not play a song.
    Unplayable,
    /// START held paused or played, away from Now Playing.
    Paused,
    Playing,
    /// SELECT with a shoulder skipped, away from Now Playing: on, or back. Short and fixed, as
    /// the corner is.
    Skipped {
        on: bool,
    },
    /// The music's keys locked, or unlocked, by the pocket combo.
    Locked(bool),
    /// Y marked the moment in a book.
    Bookmarked,
    /// The sound device stopped working; the music paused where it was.
    NoSound,
    /// The queue was kept as a playlist, or an equalizer preset as one's own.
    Saved,
    /// One's own equalizer preset deleted.
    Deleted,
    /// A write to the card failed: it is full, or locked.
    CantSave,
}

/// Short, as the corner is: where songs went, and how many, as players say it.
pub fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Added(1) => "Added to Queue".into(),
        Notice::Added(n) => format!("Added {n} to Queue"),
        Notice::Next(1) => "Playing Next".into(),
        Notice::Next(n) => format!("Playing {n} Next"),
        Notice::Unplayable => "Can't play".into(),
        Notice::Paused => "Paused".into(),
        Notice::Playing => "Playing".into(),
        Notice::Skipped { on: true } => "Next".into(),
        Notice::Skipped { on: false } => "Previous".into(),
        Notice::Locked(true) => "Locked".into(),
        Notice::Locked(false) => "Unlocked".into(),
        Notice::Bookmarked => "Bookmarked".into(),
        Notice::NoSound => "No sound".into(),
        Notice::Saved => "Saved".into(),
        Notice::Deleted => "Deleted".into(),
        Notice::CantSave => "Can't save".into(),
    }
}

/// How long a book, or the queue, has left to play, in the words the sleep timer uses: 2 h 59
/// min left, 12 min left; a minute started counts whole, so it never says none while some is left.
pub fn time_left(secs: f64) -> String {
    let minutes = (secs.max(0.0) / 60.0).ceil() as u64;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min left"),
        (h, 0) => format!("{h} h left"),
        (h, m) => format!("{h} h {m} min left"),
    }
}

/// The queue's corner: where the cursor is in it, as a list counts, and how long it has left.
pub fn queue_corner(place: &str, secs: f64) -> String {
    format!("{place} · {}", time_left(secs))
}

/// The chapter after this one, under a book's picture, as Up Next names the next song.
pub fn up_next_chapter(title: &str) -> String {
    format!("{UP_NEXT}: {title}")
}

/// How many songs the library holds: "1,234 songs".
pub fn songs(n: usize) -> String {
    let digits = n.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{grouped} {}", if n == 1 { "song" } else { "songs" })
}

/// Minutes as a sleep timer says them: 15 min, 1 h.
fn minutes(m: u32) -> String {
    match m % 60 {
        0 if m > 0 => format!("{} h", m / 60),
        _ => format!("{m} min"),
    }
}

/// Now Playing's corner while the sleep timer runs: the time left, or that it waits for the
/// end.
pub fn sleeping(left: Option<Duration>) -> String {
    match left {
        Some(left) => format!("Sleep {}", clock(left.as_secs_f64().ceil())),
        None => "Sleep at End".into(),
    }
}

/// A name and who made it, as Up Next names them: the lyrics' header, and what plays under Now
/// Playing and Continue; the name alone when who made it is not known.
pub fn name_by(title: &str, artist: &str) -> String {
    if artist.is_empty() {
        title.into()
    } else {
        format!("{title} · {artist}")
    }
}

/// The track after this one, under Now Playing.
pub fn up_next(title: &str, artist: &str) -> String {
    format!("{UP_NEXT}: {title} · {artist}")
}

/// A book's menu row that marks it finished, or not.
pub fn finished(finished: bool) -> &'static str {
    if finished {
        "Mark as Unfinished"
    } else {
        "Mark as Finished"
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

/// A decade, as Years lists them: 1990–99, which reads as well in capitals as not.
pub fn decade(start: u32) -> String {
    format!("{start}–{:02}", (start + 9) % 100)
}

/// Who made an album and when, on its second line where both are wanted: Aurora Lane · 2019.
pub fn by_in(artist: &str, year: u32) -> String {
    format!("{artist} · {year}")
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
/// The audiobooks' menu: the latest book plays on, and the lists it opens.
pub const CONTINUE: &str = "Continue";
const IN_PROGRESS: &str = "In Progress";
const AUTHORS: &str = "Authors";
const ALL_BOOKS: &str = "All Books";

/// The library being read, and how far, once the songs are counted.
pub fn reading(done: usize, of: usize) -> String {
    match of {
        0 => "Reading songs".into(),
        of => format!("Reading songs {done}/{of}"),
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

/// The corner while album pictures are made after a scan, as players say it: artwork.
pub fn artwork(done: usize, of: usize) -> String {
    format!("Artwork {done}/{of}")
}

/// Marks the hint drawn as Left and Right arrows rather than a button name.
pub const ARROWS: &str = "LEFT RIGHT";

/// The cap a key's hint shows it on; none for a key no hint names.
pub(crate) fn cap(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::A => "A",
        Key::B => "B",
        Key::X => "X",
        Key::Y => "Y",
        Key::L1 => "L1",
        Key::R1 => "R1",
        Key::L2 => "L2",
        Key::R2 => "R2",
        Key::Start => "START",
        Key::Select => "SELECT",
        Key::Left | Key::Right => ARROWS,
        _ => return None,
    })
}

/// Which of `legend`'s hints is `key`'s, if it has one.
pub fn hint_of(legend: Legend, key: Key) -> Option<usize> {
    let cap = cap(key)?;
    self::legend(legend).iter().position(|h| h.key == cap)
}

/// A button hint: the button, what a tap does, and what holding it does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Hint {
    pub key: &'static str,
    pub tap: Option<&'static str>,
    pub held: Option<&'static str>,
}

impl Hint {
    /// What it says: the tap, and the hold in brackets, "Play (Next)", or only "(Delete)".
    pub fn label(&self) -> String {
        match (self.tap, self.held) {
            (Some(tap), Some(held)) => format!("{tap} ({held})"),
            (None, Some(held)) => format!("({held})"),
            (tap, None) => tap.unwrap_or_default().into(),
        }
    }
}

/// A word a hint says, named for the screens' key tables, which hold no words of their own.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Word {
    Back,
    Open,
    Shuffle,
    Resume,
    Playing,
    Play,
    Pause,
    Next,
    PlayNext,
    PlayLine,
    Chapters,
    Update,
    Bookmarks,
    Bookmark,
    Speed,
    List,
    AddToQueue,
    Favorite,
    Restart,
    Remove,
    Move,
    Cancel,
    Drop,
    Lyrics,
    Close,
    /// How far an audiobook jumps, in seconds.
    Jump(u32),
    Change,
    Rebuild,
    /// The equalizer's: use a preset, keep one's own, delete it.
    Choose,
    Save,
    Delete,
}

fn word(word: Word) -> &'static str {
    match word {
        Word::Back => "Back",
        Word::Open => "Open",
        Word::Shuffle => "Shuffle",
        Word::Resume => "Resume",
        Word::Playing => "Playing",
        Word::Play => "Play",
        Word::Pause => "Pause",
        Word::Next => "Next",
        Word::PlayNext => "Play Next",
        Word::PlayLine => "Play Line",
        Word::Chapters => CHAPTERS,
        Word::Update => "Update",
        Word::Bookmarks => BOOKMARKS,
        Word::Bookmark => "Bookmark",
        Word::Speed => "Speed",
        Word::List => "List",
        Word::AddToQueue => "Add to Queue",
        Word::Favorite => "Favorite",
        Word::Restart => "Restart",
        Word::Remove => "Remove",
        Word::Move => "Move",
        Word::Cancel => "Cancel",
        Word::Drop => "Drop",
        Word::Lyrics => LYRICS,
        Word::Close => "Close",
        Word::Jump(10) => "10 s",
        Word::Jump(15) => "15 s",
        Word::Jump(_) => "30 s",
        Word::Change => "Change",
        Word::Rebuild => "Rebuild",
        Word::Choose => "Choose",
        Word::Save => "Save",
        Word::Delete => "Delete",
    }
}

/// A screen's button hints, as its key table has them, in its words.
pub fn legend(legend: Legend) -> Vec<Hint> {
    binds(legend)
        .into_iter()
        .filter_map(|bind| {
            let (tap, held) = bind.hint?;
            Some(Hint {
                key: cap(bind.key)?,
                tap: tap.map(word),
                held: held.map(word),
            })
        })
        .collect()
}

/// A list of audiobooks, by what it holds.
fn book_list(list: BookList) -> &'static str {
    match list {
        BookList::Listening => IN_PROGRESS,
        BookList::All => ALL_BOOKS,
        BookList::Finished => FINISHED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::{MainHold, Target};

    #[test]
    fn a_song_count_reads_in_thousands() {
        assert_eq!(songs(1), "1 song");
        assert_eq!(songs(999), "999 songs");
        assert_eq!(songs(1234), "1,234 songs");
        assert_eq!(songs(1_234_567), "1,234,567 songs");
    }

    #[test]
    fn a_held_key_s_ring_goes_round_its_own_hint_or_in_the_corner_without_one() {
        let main = Legend::Main {
            playing: true,
            hold: Some(MainHold::Shuffle),
        };
        let hints = legend(main);
        let a = hint_of(main, Key::A).unwrap();
        assert_eq!(hints[a].key, "A", "A held, round A");
        assert_eq!(
            hint_of(main, Key::Pocket),
            None,
            "the pocket combo has no hint"
        );
        assert_eq!(
            hint_of(Legend::List(Target::Album), Key::Start),
            None,
            "nor START, held, on an album's list, which has no menu"
        );
    }
}
