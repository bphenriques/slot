//! What the player tells programs outside it, each turned on in Settings › Advanced and off
//! by default. Each is told a song as it counts as listened to, a `Listen`, and says what to
//! write for it; the shell writes it, off the frame, as it does every file.

pub(crate) mod scrobbler;

/// A song listened to, as an integration is told it: its tags, how long it is, and when it
/// began, in seconds since 1970.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Listen {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub number: String,
    pub secs: f64,
    pub began: u64,
}
