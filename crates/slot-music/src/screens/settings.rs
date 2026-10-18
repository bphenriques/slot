//! Music Settings: Left, Right and A change the highlighted setting, as in the quick menu.

use super::{list, Screen};
use crate::browse::{Key, Outcome};
use crate::settings::Setting;
use crate::Music;

pub(crate) fn press(setting: Setting, key: Key) -> Option<Outcome> {
    let by = match key {
        Key::Left => -1,
        Key::Right => 1,
        Key::A => 0,
        _ => return None,
    };
    Some(Outcome::Change(setting, by))
}

pub(crate) fn screen(music: &Music) -> Screen {
    list::rows_screen(music)
}
