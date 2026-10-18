//! The screen off, with music playing on: the keys a closed lid leaves, the shoulders and
//! triggers, as Now Playing has them. L1 and R1 change song, or a book's chapter; L2 and R2
//! seek, or jump in a book; held, they seek on. Play and pause are the pocket combo's.

use super::now_playing;
use crate::browse::keys::{Mode, Press};
use crate::browse::{Key, Outcome};

/// As on Now Playing: tapped, they act as they let go, so the first of a pocket combo does
/// nothing; held, they seek.
pub(crate) fn mode(key: Key) -> Mode {
    match key {
        Key::L1 | Key::R1 | Key::L2 | Key::R2 => now_playing::mode(key),
        _ => Mode::Once,
    }
}

/// `jump` is how far an audiobook jumps, while one plays.
pub(crate) fn act(press: Press<Key>, jump: Option<i32>) -> Outcome {
    let seek = jump.unwrap_or(now_playing::SEEK_S);
    match press {
        Press::Tap(Key::L1) => Outcome::Skip(-1),
        Press::Tap(Key::R1) => Outcome::Skip(1),
        Press::Tap(Key::L2) => Outcome::Seek(-seek),
        Press::Tap(Key::R2) => Outcome::Seek(seek),
        Press::Held(key) => now_playing::held(key),
        Press::Tap(_) => Outcome::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lid_closed_keys_are_now_playing_s_shoulders_and_triggers() {
        assert_eq!(
            act(Press::Tap(Key::R1), None),
            Outcome::Skip(1),
            "next song"
        );
        assert_eq!(
            act(Press::Tap(Key::L1), Some(30)),
            Outcome::Skip(-1),
            "or chapter"
        );
        assert_eq!(act(Press::Tap(Key::R2), None), Outcome::Seek(10));
        assert_eq!(
            act(Press::Tap(Key::L2), Some(30)),
            Outcome::Seek(-30),
            "a book jumps"
        );
        assert_eq!(
            act(Press::Held(Key::R1), None),
            Outcome::Seek(10),
            "held, seek on"
        );
        assert_eq!(
            act(Press::Tap(Key::A), None),
            Outcome::None,
            "the rest are under the lid"
        );
        assert_eq!(mode(Key::R2), Mode::HoldRepeat);
    }
}
