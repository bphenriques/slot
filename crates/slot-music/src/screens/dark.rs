//! The screen off, by POWER or a closed lid, with music playing on: the keys found blind, the
//! shoulders and triggers. L1 and R1 change song, as on Now Playing, and held they seek; L2 and
//! R2 play or pause, as A would, out of reach in a pocket or under a lid, and as a headphone's
//! button does. In a book,
//! what is wanted on the go: L1 and R1 jump back or on, to hear again what was missed, and a
//! trigger held marks the moment.

use super::now_playing;
use crate::browse::keys::Press;
use crate::browse::{Key, Outcome};

/// `jump` is how far an audiobook jumps, while one plays.
pub(crate) fn act(press: Press<Key>, jump: Option<i32>) -> Outcome {
    match (press, jump) {
        (Press::Tap(Key::L1), Some(jump)) => Outcome::Seek(-jump),
        (Press::Tap(Key::R1), Some(jump)) => Outcome::Seek(jump),
        (Press::Tap(Key::L1), None) => Outcome::Skip(-1),
        (Press::Tap(Key::R1), None) => Outcome::Skip(1),
        (Press::Tap(Key::L2 | Key::R2), _) => Outcome::Toggle,
        (Press::Held(key @ (Key::L1 | Key::R1)), _) => now_playing::held(key),
        (Press::Held(Key::L2 | Key::R2), Some(_)) => Outcome::Bookmark,
        (Press::Tap(_) | Press::Held(_), _) => Outcome::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::keys::Mode;
    use crate::screens::binds;

    #[test]
    fn under_the_lid_the_shoulders_change_song_and_seek_and_the_triggers_play_or_pause() {
        assert_eq!(act(Press::Tap(Key::R1), None), Outcome::Skip(1), "next");
        assert_eq!(act(Press::Tap(Key::L1), None), Outcome::Skip(-1), "or back");
        assert_eq!(
            act(Press::Held(Key::R1), None),
            Outcome::Seek(now_playing::SEEK_S),
            "held, seek on"
        );
        assert_eq!(act(Press::Tap(Key::L2), None), Outcome::Toggle);
        assert_eq!(act(Press::Tap(Key::R2), None), Outcome::Toggle);
        assert_eq!(act(Press::Held(Key::R2), None), Outcome::None);
        assert_eq!(
            act(Press::Tap(Key::A), None),
            Outcome::None,
            "the rest are out of reach"
        );
        // A trigger acts as it is let go, so it may yet be one of the two that lock.
        let dark = binds::dark(false);
        assert_eq!(binds::mode(&dark, Key::R2), Mode::Release);
        assert_eq!(binds::mode(&dark, Key::R1), Mode::HoldRepeat);
    }

    #[test]
    fn in_a_book_the_shoulders_jump_and_a_trigger_held_marks_the_moment() {
        assert_eq!(
            act(Press::Tap(Key::L1), Some(30)),
            Outcome::Seek(-30),
            "hear it again"
        );
        assert_eq!(act(Press::Tap(Key::R1), Some(30)), Outcome::Seek(30));
        assert_eq!(act(Press::Tap(Key::R2), Some(30)), Outcome::Toggle);
        assert_eq!(act(Press::Held(Key::L2), Some(30)), Outcome::Bookmark);
        assert_eq!(binds::mode(&binds::dark(true), Key::L2), Mode::Hold);
    }
}
