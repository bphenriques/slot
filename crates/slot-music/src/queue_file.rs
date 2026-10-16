//! The queue on the card, to outlast a reboot or rescan: a plain playlist, plus two comment
//! lines for the place in it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use crate::library::Library;

pub const QUEUE_FILE: &str = "Config/music-queue.m3u8";
const AT: &str = "#SLOT-AT:";
const POS: &str = "#SLOT-POS:";

/// The file's text, with paths from the card's root.
pub fn encode(root: &Path, lib: &Library, queue: &[usize], at: usize, pos: f64) -> String {
    let mut out = format!("#EXTM3U\n{AT}{at}\n{POS}{pos:.1}\n");
    for &t in queue {
        let path = &lib.tracks[t].path;
        out.push_str(&path.strip_prefix(root).unwrap_or(path).to_string_lossy());
        out.push('\n');
    }
    out
}

/// Whether a queue is saved on the card, without the library to read it by.
/// Read as far as the first song only: it is asked on the way in.
pub fn saved(root: &Path) -> bool {
    use std::io::BufRead;
    std::fs::File::open(root.join(QUEUE_FILE)).is_ok_and(|file| {
        std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .any(|l| !l.trim().is_empty() && !l.starts_with('#'))
    })
}

/// The queue from the file's text. Tracks no longer in the library are dropped.
pub fn decode(text: &str, root: &Path, lib: &Library) -> (Vec<usize>, usize, f64) {
    let index: HashMap<&Path, usize> = lib
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.path.as_path(), i))
        .collect();
    let (mut at, mut pos) = (0, 0.0);
    let mut entries = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(v) = line.strip_prefix(AT) {
            at = v.parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix(POS) {
            pos = v.parse().unwrap_or(0.0);
        } else if !line.starts_with('#') {
            entries.push(index.get(root.join(line).as_path()).copied());
        }
    }
    // Where the place lands among the kept tracks, and whether its track was kept.
    let kept_before = entries[..at.min(entries.len())].iter().flatten().count();
    let still_there = entries.get(at).is_some_and(Option::is_some);
    let queue: Vec<usize> = entries.into_iter().flatten().collect();
    match (still_there, kept_before < queue.len()) {
        (true, _) => (queue, kept_before, pos),
        (false, true) => (queue, kept_before, 0.0),
        (false, false) => (queue, 0, 0.0),
    }
}

/// Writes a file off the frame loop, skipping versions a newer one replaces: the queue, and
/// the audiobooks' places.
pub struct Writer {
    tx: Sender<String>,
}

impl Writer {
    pub fn spawn(path: PathBuf) -> Writer {
        let (tx, rx) = mpsc::channel::<String>();
        let spawned = std::thread::Builder::new()
            .name("slot-music-save".into())
            .spawn(move || {
                while let Ok(mut text) = rx.recv() {
                    if let Some(newer) = rx.try_iter().last() {
                        text = newer;
                    }
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(e) = slot_store::atomic_write(&path, text.as_bytes()) {
                        eprintln!("slot: music: {}: {e}", path.display());
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        Writer { tx }
    }

    pub fn save(&self, text: String) {
        let _ = self.tx.send(text);
    }
}
