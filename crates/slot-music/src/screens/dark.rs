//! The screen off, with music playing on: the keys a closed lid leaves. L1 and R1 change song,
//! or in an audiobook jump back and on, as listening blind wants; R2 plays or pauses, and held,
//! stops.

use crate::browse::{Key, Outcome};
use crate::keys::{Mode, Press};

pub(crate) fn mode(key: Key) -> Mode {
    match key {
        Key::R2 => Mode::Hold,
        _ => Mode::Once,
    }
}

/// `jump` is how far an audiobook jumps, while one plays.
pub(crate) fn act(press: Press<Key>, jump: Option<i32>) -> Outcome {
    match (press, jump) {
        (Press::Tap(Key::L1), Some(jump)) => Outcome::Seek(-jump),
        (Press::Tap(Key::R1), Some(jump)) => Outcome::Seek(jump),
        (Press::Tap(Key::L1), None) => Outcome::Skip(-1),
        (Press::Tap(Key::R1), None) => Outcome::Skip(1),
        (Press::Tap(Key::R2), _) => Outcome::Toggle,
        (Press::Held(Key::R2), _) => Outcome::Stop,
        _ => Outcome::None,
    }
}
