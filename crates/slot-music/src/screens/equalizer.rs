//! The equalizer: Off, slot's presets and one's own, A choosing one, X held deleting one's own;
//! and New Preset, its bands moved by Left and Right as settings are, heard as they move, Y
//! keeping them as one's own. B leaves it unkept, and the preset chosen plays again.

use crate::browse::{Context, Key, Outcome};

/// On a preset, A chooses it.
pub(crate) fn press_preset(i: usize, key: Key) -> Option<Outcome> {
    (key == Key::A).then_some(Outcome::Equalizer(i))
}

/// Held on one's own preset, X deletes it; slot's stay.
pub(crate) fn held(cx: &Context, i: usize, key: Key) -> Outcome {
    let own = cx.equalizer.get(i).is_some_and(|p| p.own);
    if key == Key::X && own {
        Outcome::DeletePreset(i)
    } else {
        Outcome::None
    }
}

/// New Preset's row opens it, from the preset chosen.
pub(crate) fn press_new(key: Key) -> Option<Outcome> {
    (key == Key::A).then_some(Outcome::NewPreset)
}

/// On a band, Left and Right move it a dB; Y keeps them all.
pub(crate) fn press_band(band: usize, key: Key) -> Option<Outcome> {
    match key {
        Key::Left => Some(Outcome::Tune { band, by: -1 }),
        Key::Right => Some(Outcome::Tune { band, by: 1 }),
        Key::Y => Some(Outcome::SavePreset),
        _ => None,
    }
}
