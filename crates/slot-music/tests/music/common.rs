//! What every area's tests share: a library made of empty files, and the ways in.
#![allow(dead_code)]

pub use std::path::{Path, PathBuf};

pub use slot_music::store::queue::SavedQueue;
pub use slot_music::store::{Card, Kept, Store};
pub use slot_music::{
    text::Notice, track_title, AlbumId, ArtistId, BookId, Browser, Chapter, Command, Context,
    Event, Key, Library, Mode, Outcome, PlayState, Player, Queue, QueueView, Recent, Settings,
    Shuffle, TrackId, View,
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
        ..Context::new(lib)
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

// The main screen: Now Playing, Playlists, Artists, Albums, Songs, Settings. Artists and Albums
// open on Play All and Shuffle, so these go on to the first of them.
pub const TO_ARTISTS: [Key; 5] = [Key::Down, Key::Down, Key::A, Key::Down, Key::Down];

pub const TO_ALBUMS: [Key; 6] = [
    Key::Down,
    Key::Down,
    Key::Down,
    Key::A,
    Key::Down,
    Key::Down,
];

pub const TO_SETTINGS: [Key; 2] = [Key::Up, Key::A];

pub const TO_TRACKS: [Key; 5] = [Key::Down, Key::Down, Key::Down, Key::Down, Key::A];

pub fn played(c: Option<Command>) -> PathBuf {
    match c {
        Some(Command::Play { path, .. }) => path,
        other => panic!("{other:?}"),
    }
}

pub fn record(path: &str, tags: slot_music::Tags) -> slot_music::Record {
    slot_music::Record {
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
        "../Audiobooks/Wilde - The Happy Prince.m4b",
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

/// The player heard `at` seconds into a file `len` long, in whatever state it is.
pub fn heard(p: &mut Player, at: f64, len: f64) {
    let state = p.state();
    p.observe(slot_music::PlayerStatus { state, at, len });
}

/// The player says it is `state`, where it was.
pub fn reported(p: &mut Player, state: PlayState) {
    let (at, len) = p.time();
    p.observe(slot_music::PlayerStatus { state, at, len });
}
