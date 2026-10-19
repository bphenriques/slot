//! The queue on the card, to outlast a reboot or rescan: a plain playlist, plus comment lines
//! for the place in it and, shuffled, the order before.

use std::path::Path;

use super::{Card, Kept};
use crate::library::TrackId;
use crate::playback::queue::Queue;

const AT: &str = "#SLOT-AT:";
const POS: &str = "#SLOT-POS:";
/// Shuffled, the order before: each song's place in the list below, from 0.
const ORDER: &str = "#SLOT-ORDER:";

/// The music's queue as it was left: its tracks, the place in it and how far into that track,
/// and, shuffled, the order before, as `Queue::original` has it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SavedQueue {
    pub tracks: Vec<TrackId>,
    pub at: usize,
    pub pos: f64,
    pub original: Option<Vec<usize>>,
}

impl SavedQueue {
    pub fn of(queue: &Queue, pos: f64) -> SavedQueue {
        SavedQueue {
            tracks: queue.tracks().to_vec(),
            at: queue.at(),
            pos,
            original: queue.original().map(<[usize]>::to_vec),
        }
    }

    /// The queue to play, shuffled as it was.
    pub fn queue(self) -> Queue {
        Queue::restored(self.tracks, self.at, self.original)
    }
}

impl Kept for SavedQueue {
    const FILE: &'static str = "Config/music-queue.m3u8";

    /// With paths from the card's root.
    fn encode(&self, card: Card) -> String {
        let mut out = format!("#EXTM3U\n{AT}{}\n{POS}{:.1}\n", self.at, self.pos);
        if let Some(original) = &self.original {
            let order: Vec<String> = original.iter().map(usize::to_string).collect();
            out.push_str(&format!("{ORDER}{}\n", order.join(",")));
        }
        for &t in &self.tracks {
            out.push_str(&card.path(t));
            out.push('\n');
        }
        out
    }

    /// Tracks no longer in the library are dropped, and the place and the order before move
    /// with the rest.
    fn decode(text: &str, card: Card) -> SavedQueue {
        let index = card.lib.index();
        let (mut at, mut pos, mut order) = (0, 0.0, None);
        let mut entries = Vec::new();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if let Some(v) = line.strip_prefix(AT) {
                at = v.parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix(POS) {
                pos = v.parse().unwrap_or(0.0);
            } else if let Some(v) = line.strip_prefix(ORDER) {
                order = v
                    .split(',')
                    .map(|p| p.parse::<usize>().ok())
                    .collect::<Option<Vec<_>>>();
            } else if !line.starts_with('#') {
                entries.push(index.get(card.root.join(line).as_path()).copied());
            }
        }
        // Each entry's place among the kept ones, if kept.
        let mut kept = 0;
        let place: Vec<Option<usize>> = entries
            .iter()
            .map(|e| {
                e.map(|_| {
                    kept += 1;
                    kept - 1
                })
            })
            .collect();
        let original = order.map(|o| {
            o.into_iter()
                .filter_map(|p| place.get(p).copied().flatten())
                .collect()
        });
        // Where the place lands among the kept tracks, and whether its track was kept.
        let kept_before = entries[..at.min(entries.len())].iter().flatten().count();
        let still_there = entries.get(at).is_some_and(Option::is_some);
        let tracks: Vec<TrackId> = entries.into_iter().flatten().collect();
        let (at, pos) = match (still_there, kept_before < tracks.len()) {
            (true, _) => (kept_before, pos),
            (false, true) => (kept_before, 0.0),
            (false, false) => (0, 0.0),
        };
        SavedQueue {
            tracks,
            at,
            pos,
            original,
        }
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
