//! Music Settings: Left, Right and A change the highlighted setting, as in the quick menu.

use super::{list, Legend, Screen};
use crate::browse::rows::Row;
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
    // Rebuild Library acts only when A is held, and says so.
    list::rows_screen(music, |row| match row {
        Some(Row::Rebuild) => Legend::Hold,
        // A opens the Sound page.
        Some(Row::Sound | Row::BookSettings | Row::Debug) => Legend::Menu,
        Some(Row::TestBuzz) => Legend::Test,
        // Only told, not changed.
        Some(Row::LastError) => Legend::Back,
        _ => Legend::Settings,
    })
}
