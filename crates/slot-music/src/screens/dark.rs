//! The screen off, with music playing on: the keys a closed lid leaves, the shoulders and
//! triggers. L1 and R1 change song, or a book's chapter, as on Now Playing, and held they seek;
//! L2 and R2 play or pause, as A would, out of reach under the lid, and as a headphone's button
//! does.

use super::now_playing;
use crate::browse::keys::Press;
use crate::browse::{Key, Outcome};

pub(crate) fn act(press: Press<Key>) -> Outcome {
    match press {
        Press::Tap(Key::L1) => Outcome::Skip(-1),
        Press::Tap(Key::R1) => Outcome::Skip(1),
        Press::Tap(Key::L2 | Key::R2) => Outcome::Toggle,
        Press::Held(key @ (Key::L1 | Key::R1)) => now_playing::held(key),
        Press::Tap(_) | Press::Held(_) => Outcome::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::keys::Mode;
    use crate::screens::binds;

    #[test]
    fn under_the_lid_the_shoulders_change_song_and_seek_and_the_triggers_play_or_pause() {
        assert_eq!(act(Press::Tap(Key::R1)), Outcome::Skip(1), "next");
        assert_eq!(act(Press::Tap(Key::L1)), Outcome::Skip(-1), "or back");
        assert_eq!(
            act(Press::Held(Key::R1)),
            Outcome::Seek(now_playing::SEEK_S),
            "held, seek on"
        );
        assert_eq!(act(Press::Tap(Key::L2)), Outcome::Toggle);
        assert_eq!(act(Press::Tap(Key::R2)), Outcome::Toggle);
        assert_eq!(
            act(Press::Tap(Key::A)),
            Outcome::None,
            "the rest are under the lid"
        );
        // A trigger acts as it is let go, so it may yet be one of the two that lock.
        let dark = binds::dark();
        assert_eq!(binds::mode(&dark, Key::R2), Mode::Release);
        assert_eq!(binds::mode(&dark, Key::R1), Mode::HoldRepeat);
    }
}
