//! A scrobbler log, as Rockbox keeps one: `.scrobbler.log` at the card's root, a line a song
//! listened to, which a Last.fm uploader reads later, the device being offline. A song counts as
//! Last.fm counts it: over 30 seconds long, and played half way or 4 minutes, whichever is first.

use super::Listen;
use crate::library::TrackId;

pub const FILE: &str = ".scrobbler.log";
/// What the file begins with, as the format has it: its version, the time zone of the times in
/// it, and what wrote it.
pub const HEADER: &str = "#AUDIOSCROBBLER/1.1\n#TZ/UTC\n#CLIENT/slot\n";

/// Songs shorter than this are not scrobbled; past this much heard, a longer one is.
const SHORTEST: f64 = 30.0;
const ENOUGH: f64 = 240.0;

/// Whether the song playing has been told yet, this time it plays.
#[derive(Default)]
pub(crate) struct Scrobbler {
    told: Option<TrackId>,
}

impl Scrobbler {
    /// Song `t` is `at` seconds into its `len`, `playing` or not: true once, the moment it
    /// counts. Played again from its start, it counts again.
    pub fn hear(&mut self, t: TrackId, at: f64, len: f64, playing: bool) -> bool {
        if at < 1.0 && self.told == Some(t) {
            self.told = None;
        }
        let counts = len > SHORTEST && at >= (len / 2.0).min(ENOUGH);
        if !playing || !counts || self.told == Some(t) {
            return false;
        }
        self.told = Some(t);
        true
    }
}

/// `listen`'s line: tab between each of artist, album, title, its number, its length in
/// seconds, L for listened, and when it began; and an empty MusicBrainz id.
pub fn line(listen: &Listen) -> String {
    // A tab or a line break in a tag would break the line apart.
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    format!(
        "{}\t{}\t{}\t{}\t{}\tL\t{}\t\n",
        clean(&listen.artist),
        clean(&listen.album),
        clean(&listen.title),
        clean(&listen.number),
        listen.secs.round() as u64,
        listen.began,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_counts_once_half_way_or_four_minutes_in_and_again_played_from_the_start() {
        let mut s = Scrobbler::default();
        let t = TrackId(1);
        assert!(!s.hear(t, 50.0, 200.0, true), "not yet half way");
        assert!(s.hear(t, 100.0, 200.0, true), "half way");
        assert!(!s.hear(t, 150.0, 200.0, true), "once");
        assert!(!s.hear(t, 0.5, 200.0, true));
        assert!(
            s.hear(t, 100.0, 200.0, true),
            "again, played from its start"
        );
        let long = TrackId(2);
        assert!(
            s.hear(long, 240.0, 1200.0, true),
            "four minutes of a long one"
        );
        assert!(!s.hear(TrackId(3), 20.0, 25.0, true), "too short to count");
        assert!(!s.hear(TrackId(4), 150.0, 200.0, false), "not while paused");
    }

    #[test]
    fn a_line_is_as_rockbox_writes_it_its_tags_kept_on_the_one_line() {
        let listen = Listen {
            artist: "Abba".into(),
            album: "Gold".into(),
            title: "S.O.S\tlive".into(),
            number: "03".into(),
            secs: 201.6,
            began: 1_700_000_000,
        };
        assert_eq!(
            line(&listen),
            "Abba\tGold\tS.O.S live\t03\t202\tL\t1700000000\t\n"
        );
        assert!(HEADER.starts_with("#AUDIOSCROBBLER/1.1\n#TZ/UTC\n"));
    }
}
