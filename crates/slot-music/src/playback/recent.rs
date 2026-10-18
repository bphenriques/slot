//! Recently Played: the songs last listened to, newest first, each once at its latest play, as
//! iTunes and Symfonium keep them. A song counts once it has played a while, so a skip does
//! not; a book's parts never do.

/// How many songs it keeps.
const KEEP: usize = 100;
/// How long a song plays before it counts; a shorter one counts a second before its end.
const COUNTS_AFTER: f64 = 30.0;

use crate::library::TrackId;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Recent {
    pub tracks: Vec<TrackId>,
    /// The song last counted, so a play counts once; starting it over counts again.
    counted: Option<TrackId>,
}

impl Recent {
    pub fn new(tracks: Vec<TrackId>) -> Recent {
        Recent {
            tracks,
            counted: None,
        }
    }

    /// Song `t` is `at` seconds into its `len`, `playing` or not. True when that counts it and
    /// the list changed, to be written.
    pub fn hear(&mut self, t: TrackId, at: f64, len: f64, playing: bool) -> bool {
        if at < 1.0 && self.counted == Some(t) {
            self.counted = None;
        }
        let long = len > 0.0 && at >= COUNTS_AFTER.min(len - 1.0);
        if !playing || !long || self.counted == Some(t) {
            return false;
        }
        self.counted = Some(t);
        self.played(t)
    }

    /// Puts `t` first; false when it was first already, so nothing needs writing.
    fn played(&mut self, t: TrackId) -> bool {
        if self.tracks.first() == Some(&t) {
            return false;
        }
        self.tracks.retain(|&r| r != t);
        self.tracks.insert(0, t);
        self.tracks.truncate(KEEP);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_counts_once_it_has_played_a_while_and_a_skip_does_not() {
        let mut recent = Recent::default();
        assert!(!recent.hear(TrackId(1), 10.0, 200.0, true), "skipped early");
        assert!(!recent.hear(TrackId(1), 40.0, 200.0, false), "paused");
        assert!(recent.hear(TrackId(1), 30.0, 200.0, true));
        assert!(!recent.hear(TrackId(1), 31.0, 200.0, true), "once a play");
        assert!(
            recent.hear(TrackId(2), 19.0, 20.0, true),
            "a short one near its end"
        );
        assert!(
            recent.hear(TrackId(1), 30.0, 200.0, true),
            "played again, it moves up"
        );
        assert_eq!(
            recent.tracks,
            [1, 2].map(TrackId),
            "once, at its latest play"
        );
        recent.hear(TrackId(1), 0.5, 200.0, true);
        assert!(
            !recent.hear(TrackId(1), 30.0, 200.0, true),
            "first already: nothing to write"
        );
    }

    #[test]
    fn it_keeps_the_latest_hundred() {
        let mut recent = Recent::default();
        (0..200).for_each(|t| {
            recent.hear(TrackId(t), 30.0, 60.0, true);
        });
        assert_eq!(recent.tracks.len(), KEEP);
        assert_eq!(recent.tracks[0], TrackId(199));
    }
}
