//! What the worker reads for a song before it is wanted: the one playing and those either side,
//! so a song and what belongs to it, its lyrics or its chapters, change together.

use crate::library::TrackId;

/// What was read for the songs wanted, and those asked for and not read yet.
pub(crate) struct ReadAhead<T> {
    read: Vec<(TrackId, T)>,
    asked: Vec<TrackId>,
}

impl<T> Default for ReadAhead<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ReadAhead<T> {
    /// Nothing read nor asked for.
    pub const fn new() -> Self {
        ReadAhead {
            read: Vec::new(),
            asked: Vec::new(),
        }
    }

    /// What was read for song `t`, once it was.
    pub fn of(&self, t: TrackId) -> Option<&T> {
        self.read
            .iter()
            .find(|(song, _)| *song == t)
            .map(|(_, v)| v)
    }

    /// The songs wanted now, the one playing first: what was read for others is let go, and
    /// those neither read nor asked for yet are given back, to ask for. A new next song never
    /// costs the one playing what it has.
    pub fn want(&mut self, songs: &[TrackId]) -> Vec<TrackId> {
        self.read.retain(|(song, _)| songs.contains(song));
        let new: Vec<TrackId> = songs
            .iter()
            .copied()
            .filter(|&t| self.of(t).is_none() && !self.asked.contains(&t))
            .collect();
        self.asked.extend(&new);
        new
    }

    /// What was read for `t`.
    pub fn put(&mut self, t: TrackId, value: T) {
        self.asked.retain(|&song| song != t);
        self.read.retain(|(song, _)| *song != t);
        self.read.push((t, value));
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
    fn what_is_wanted_is_asked_for_once_and_the_song_playing_keeps_its_own() {
        let mut ahead = ReadAhead::default();
        let (one, two, three) = (TrackId(1), TrackId(2), TrackId(3));
        assert_eq!(ahead.want(&[one, two]), [one, two], "the song and the next");
        assert_eq!(ahead.want(&[one, two]), [], "not twice while they are read");
        ahead.put(one, "one");
        ahead.put(two, "two");
        // A shuffle deals another next: only it is asked for, and the song keeps its own.
        assert_eq!(ahead.want(&[one, three]), [three]);
        assert_eq!(
            ahead.of(one),
            Some(&"one"),
            "the song playing keeps what it has"
        );
        assert_eq!(ahead.of(two), None, "the next no longer wanted is let go");
        ahead.clear();
        assert_eq!(ahead.want(&[one]), [one], "asked again after a rescan");
    }
}
