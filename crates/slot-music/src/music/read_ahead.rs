//! What the worker reads for a song before it is wanted: the one playing and the one after it,
//! so a song and what belongs to it, its lyrics or its chapters, change together.

use crate::library::TrackId;

/// What was read for the last few songs, and those it was asked for and not read yet.
pub(crate) struct ReadAhead<T> {
    read: Vec<(TrackId, T)>,
    asked: Vec<TrackId>,
    keep: usize,
}

impl<T> ReadAhead<T> {
    /// Keeping what was read for `keep` songs.
    pub fn new(keep: usize) -> ReadAhead<T> {
        ReadAhead {
            read: Vec::new(),
            asked: Vec::new(),
            keep,
        }
    }

    /// What was read for song `t`, once it was.
    pub fn of(&self, t: TrackId) -> Option<&T> {
        self.read
            .iter()
            .find(|(song, _)| *song == t)
            .map(|(_, v)| v)
    }

    /// Whether `t` is still to be asked for: neither read nor asked yet. Asking is the caller's.
    pub fn ask(&mut self, t: TrackId) -> bool {
        let wants = self.of(t).is_none() && !self.asked.contains(&t);
        if wants {
            self.asked.push(t);
        }
        wants
    }

    /// What was read for `t`, the oldest let go past `keep`.
    pub fn put(&mut self, t: TrackId, value: T) {
        self.asked.retain(|&song| song != t);
        self.read.retain(|(song, _)| *song != t);
        self.read.push((t, value));
        if self.read.len() > self.keep {
            self.read.remove(0);
        }
    }

    /// Forgets everything: the library was read again, and songs renumbered.
    pub fn clear(&mut self) {
        self.read.clear();
        self.asked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_is_asked_for_once_and_the_oldest_read_go_first() {
        let mut ahead = ReadAhead::new(2);
        assert!(ahead.ask(TrackId(1)), "asked");
        assert!(!ahead.ask(TrackId(1)), "not twice while it is read");
        ahead.put(TrackId(1), "one");
        assert!(!ahead.ask(TrackId(1)), "nor once read");
        ahead.put(TrackId(2), "two");
        ahead.put(TrackId(3), "three");
        assert_eq!(ahead.of(TrackId(1)), None, "the oldest let go");
        assert_eq!(ahead.of(TrackId(3)), Some(&"three"));
        ahead.clear();
        assert!(ahead.ask(TrackId(3)), "asked again after a rescan");
    }
}
