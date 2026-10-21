//! What every area's tests share: a library made of empty files, and the ways in.
#![allow(dead_code)]

pub use std::path::{Path, PathBuf};

pub(crate) use crate::books::chapters::Chapter;
pub(crate) use crate::library::ArtistId;
pub(crate) use crate::playback::recent::Recent;
pub(crate) use crate::store::queue::SavedQueue;
pub(crate) use crate::store::{Card, Kept, Store};
pub(crate) use crate::{
    track_title, AlbumId, BookId, Browser, Command, Context, Event, Key, Library, Mode, Outcome,
    PlayState, Player, Queue, QueueView, Settings, Shuffle, TrackId, View,
};
pub use tempfile::TempDir;

pub fn library(files: &[&str]) -> (TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    for f in files {
        let path = d.path().join("Music").join(f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
    let lib = Library::scan(d.path());
    (d, lib)
}

/// What the browser reads: `lib` with `queue` and `settings`.
pub fn cx<'a>(lib: &'a Library, queue: QueueView<'a>, settings: Settings) -> Context<'a> {
    Context {
        queue,
        settings,
        ..Context::new(lib, at(0))
    }
}

pub fn names(lib: &Library) -> Vec<&str> {
    lib.artists.iter().map(|a| a.name.as_str()).collect()
}

pub fn press(b: &mut Browser, lib: &Library, keys: &[Key]) -> Outcome {
    keys.iter()
        .map(|&k| b.press(&cx(lib, QueueView::default(), Settings::default()), k))
        .last()
        .unwrap_or(Outcome::None)
}

// The main screen: Now Playing, Queue, Library, Playlists, Audiobooks, Settings; Library: Artists, Albums, Songs.
// Artists and Albums open on Play All, so these go on to the first of them.
pub const TO_ARTISTS: [Key; 5] = [Key::Down, Key::Down, Key::A, Key::A, Key::Down];

pub const TO_ALBUMS: [Key; 6] = [Key::Down, Key::Down, Key::A, Key::Down, Key::A, Key::Down];

pub const TO_SETTINGS: [Key; 2] = [Key::Up, Key::A];

pub const TO_TRACKS: [Key; 6] = [Key::Down, Key::Down, Key::A, Key::Down, Key::Down, Key::A];

pub fn played(c: Option<Command>) -> PathBuf {
    match c {
        Some(Command::Play { path, .. }) => path,
        other => panic!("{other:?}"),
    }
}

pub fn record(path: &str, tags: crate::Tags) -> crate::Record {
    crate::Record {
        path: PathBuf::from("/card/Music").join(path),
        modified: 0,
        added: 0,
        size: 0,
        tags,
    }
}

/// A library of two books, the first of two parts a minute and three minutes long.
pub fn books() -> (TempDir, Library) {
    let (d, mut lib) = library(&[
        "../Audiobooks/Kipling - Just So/01 whale.mp3",
        "../Audiobooks/Kipling - Just So/02 camel.mp3",
        "../Audiobooks/Wilde - The Selfish Giant.m4b",
    ]);
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    for (&t, secs) in parts.iter().zip([60.0, 180.0]) {
        lib.tracks[t].secs = secs;
    }
    lib.books[0].secs = 240.0;
    (d, lib)
}

/// Tracks by their numbers, as the library files them.
pub fn ids(tracks: &[usize]) -> Vec<TrackId> {
    tracks.iter().map(|&t| TrackId(t)).collect()
}

/// Chapters by where each starts and its title.
pub fn chapters(marks: &[(f64, &str)]) -> Vec<Chapter> {
    let chapter = |&(start, title): &(f64, &str)| Chapter {
        start,
        title: title.into(),
    };
    marks.iter().map(chapter).collect()
}

/// A test's own time: its first moment, and moments `ms` after it, so nothing waits on the
/// clock. Each test runs on a thread of its own, so each has its own first moment.
pub fn at(ms: u64) -> std::time::Instant {
    std::thread_local!(static FIRST: std::time::Instant = std::time::Instant::now());
    FIRST.with(|first| *first + std::time::Duration::from_millis(ms))
}

/// The player heard `at` seconds into a file `len` long, in whatever state it is.
pub fn heard(p: &mut Player, at: f64, len: f64) {
    heard_at(p, at, len, 0);
}

/// As `heard`, `ms` into the tests' time.
pub fn heard_at(p: &mut Player, at_s: f64, len: f64, ms: u64) {
    let state = p.state();
    let status = crate::PlayerStatus {
        state,
        at: at_s,
        len,
        sounding: state == PlayState::Playing,
        ..Default::default()
    };
    p.observe(status, at(ms));
}

/// The player says it is `state`, where it was, its device quiet once it is not playing.
pub fn reported(p: &mut Player, state: PlayState) {
    said(p, state, state == PlayState::Playing);
}

/// The player says it is `state`, where it was, its device still `sounding` or not: paused or
/// stopped, it sounds until its last moment has faded out.
pub fn said(p: &mut Player, state: PlayState, sounding: bool) {
    let (at_s, len) = p.time();
    let status = crate::PlayerStatus {
        state,
        at: at_s,
        len,
        sounding,
        ..Default::default()
    };
    p.observe(status, at(0));
}
