//! The queue on the card, to outlast a reboot or rescan: a plain playlist, plus two comment
//! lines for the place in it.

use std::path::Path;

use super::{Card, Kept};
use crate::library::TrackId;

const AT: &str = "#SLOT-AT:";
const POS: &str = "#SLOT-POS:";

/// The music's queue as it was left: its tracks, the place in it and how far into that track.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SavedQueue {
    pub tracks: Vec<TrackId>,
    pub at: usize,
    pub pos: f64,
}

impl Kept for SavedQueue {
    const FILE: &'static str = "Config/music-queue.m3u8";

    /// With paths from the card's root.
    fn encode(&self, card: Card) -> String {
        let mut out = format!("#EXTM3U\n{AT}{}\n{POS}{:.1}\n", self.at, self.pos);
        for &t in &self.tracks {
            out.push_str(&card.path(t));
            out.push('\n');
        }
        out
    }

    /// Tracks no longer in the library are dropped, and the place moves with the rest.
    fn decode(text: &str, card: Card) -> SavedQueue {
        let index = card.lib.index();
        let (mut at, mut pos) = (0, 0.0);
        let mut entries = Vec::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if let Some(v) = line.strip_prefix(AT) {
                at = v.parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix(POS) {
                pos = v.parse().unwrap_or(0.0);
            } else if !line.starts_with('#') {
                entries.push(index.get(card.root.join(line).as_path()).copied());
            }
        }
        // Where the place lands among the kept tracks, and whether its track was kept.
        let kept_before = entries[..at.min(entries.len())].iter().flatten().count();
        let still_there = entries.get(at).is_some_and(Option::is_some);
        let tracks: Vec<TrackId> = entries.into_iter().flatten().collect();
        let (at, pos) = match (still_there, kept_before < tracks.len()) {
            (true, _) => (kept_before, pos),
            (false, true) => (kept_before, 0.0),
            (false, false) => (0, 0.0),
        };
        SavedQueue { tracks, at, pos }
    }
}

/// Whether a queue is saved on the card, without the library to read it by.
/// Read as far as the first song only: it is asked on the way in.
pub fn saved(root: &Path) -> bool {
    use std::io::BufRead;
    std::fs::File::open(root.join(SavedQueue::FILE)).is_ok_and(|file| {
        std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .any(|l| !l.trim().is_empty() && !l.starts_with('#'))
    })
}
