//! What the player keeps on the card, and the one place that reads and writes it: the
//! settings, the queue, the books' shelf, the playlists and what was played. Each file's format
//! is here beside it, text in and text out, so the rest of the player holds plain values and
//! never touches the card. The library cache and the album pictures are made again from the
//! music, and the workers that make them keep them.

pub mod playlists;
pub mod queue;
pub mod recent;
pub mod settings;
pub mod shelf;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::library::{Library, TrackId};

/// What a file's paths are read by: the card's root, and the library on it.
#[derive(Copy, Clone)]
pub struct Card<'a> {
    pub root: &'a Path,
    pub lib: &'a Library,
}

impl Card<'_> {
    /// Track `t`'s path from the card's root, as the queue and Recently Played write it.
    pub fn path(&self, t: TrackId) -> String {
        let path = &self.lib.tracks[t].path;
        path.strip_prefix(self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }
}

/// One file kept whole: where it is on the card, and its text.
pub trait Kept: Sized {
    const FILE: &'static str;
    fn encode(&self, card: Card) -> String;
    /// What `text` says; anything it cannot read is left out.
    fn decode(text: &str, card: Card) -> Self;
}

/// The card's files: read at once, written off the frame loop, each whole or not at all.
/// Writes come in faster than the card takes them only in bursts, so a newer text of a file
/// replaces one not yet written.
pub struct Store {
    root: PathBuf,
    tx: Sender<Job>,
}

enum Job {
    /// A file's whole text.
    Whole(PathBuf, String),
    /// One `key = value` line of a settings file, the rest of it as it was.
    Key(PathBuf, &'static str, String),
    /// Says when everything sent before is written.
    Flush(Sender<()>),
}

impl Store {
    pub fn open(root: &Path) -> Store {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-music-store".into())
            .spawn(move || {
                crate::background();
                write_all(rx);
            });
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        Store {
            root: root.to_path_buf(),
            tx,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A file's text, from the card's root; empty when there is none.
    pub fn read(&self, file: impl AsRef<Path>) -> String {
        std::fs::read_to_string(self.root.join(file)).unwrap_or_default()
    }

    /// Writes a file's whole text, from the card's root, or at a path of its own.
    pub fn write(&self, file: impl AsRef<Path>, text: String) {
        let _ = self.tx.send(Job::Whole(self.root.join(file), text));
    }

    /// Sets one key of a settings file, keeping its other lines and comments.
    pub fn set(&self, file: &str, key: &'static str, value: String) {
        let _ = self.tx.send(Job::Key(self.root.join(file), key, value));
    }

    pub fn load<T: Kept>(&self, lib: &Library) -> T {
        T::decode(&self.read(T::FILE), self.card(lib))
    }

    pub fn save<T: Kept>(&self, value: &T, lib: &Library) {
        self.write(T::FILE, value.encode(self.card(lib)));
    }

    pub fn card<'a>(&'a self, lib: &'a Library) -> Card<'a> {
        Card {
            root: &self.root,
            lib,
        }
    }

    /// Waits until everything sent is written.
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Job::Flush(tx)).is_ok() {
            let _ = rx.recv();
        }
    }
}

/// Writes jobs as they come, those waiting together: a file's whole text only the newest.
fn write_all(rx: Receiver<Job>) {
    while let Ok(first) = rx.recv() {
        let jobs: Vec<Job> = std::iter::once(first).chain(rx.try_iter()).collect();
        for (i, job) in jobs.iter().enumerate() {
            let newer = |path: &Path| {
                jobs[i + 1..]
                    .iter()
                    .any(|j| matches!(j, Job::Whole(p, _) if p == path))
            };
            let done = match job {
                Job::Whole(path, _) if newer(path) => Ok(()),
                Job::Whole(path, text) => whole(path, text),
                Job::Key(path, key, value) => set_key(path, key, value),
                Job::Flush(done) => {
                    let _ = done.send(());
                    Ok(())
                }
            };
            if let Err(e) = done {
                eprintln!("slot: music: {e}");
            }
        }
    }
}

fn whole(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    slot_store::atomic_write(path, text.as_bytes())
        .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

fn set_key(path: &Path, key: &str, value: &str) -> std::io::Result<()> {
    let was = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    whole(path, &slot_store::ini::set(&was, key, Some(value))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_land_whole_and_in_order_and_a_newer_text_wins() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path());
        store.write("Config/a.txt", "one".into());
        store.write("Config/a.txt", "two".into());
        store.set("Config/b.txt", "x", "1".into());
        store.set("Config/b.txt", "y", "2".into());
        store.flush();
        assert_eq!(store.read("Config/a.txt"), "two");
        let b = slot_store::ini::parse(&store.read("Config/b.txt"));
        assert_eq!((b["x"].as_str(), b["y"].as_str()), ("1", "2"));
        assert_eq!(store.read("Config/none.txt"), "", "nothing there");
    }
}
