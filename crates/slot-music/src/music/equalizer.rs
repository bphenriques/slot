//! The equalizer's presets chosen, made and deleted: one's own kept in their file, the preset
//! chosen in `music.txt` by name. What is heard is worked out each frame (`tell_sound`): New
//! Preset's bands while it is open, else the preset chosen, so leaving it any way puts that back.

use super::{Effect, Music};
use crate::browse::View;
use crate::store::equalizer::{Gains, EQ_FILE, MOST_DB};
use crate::text::{self, Notice};

impl Music {
    /// Plays through preset `i`, from 1, or none for 0.
    pub(super) fn choose_preset(&mut self, i: usize) {
        self.settings.eq = i;
        let name = self.equalizer.name(i).unwrap_or("off").to_string();
        self.out.push(Effect::Setting("eq", name));
    }

    /// Opens New Preset, its bands where the preset chosen has them.
    pub(super) fn new_preset(&mut self) {
        self.tuning = self.equalizer.gains(self.settings.eq);
        self.browser.push(View::NewPreset, 0);
    }

    /// Moves New Preset's `band` by `by` dB, as far as it goes.
    pub(super) fn tune(&mut self, band: usize, by: i32) {
        let db = &mut self.tuning[band];
        *db = (db.round() + by as f32).clamp(-MOST_DB, MOST_DB);
    }

    /// Keeps New Preset's bands as one's own preset, named for how many came before, and plays
    /// through it; back on the presets, at it.
    pub(super) fn save_preset(&mut self) {
        let name = (1..)
            .map(text::own_preset)
            .find(|name| self.equalizer.find(name) == 0)
            .unwrap_or_default();
        let at = self.equalizer.keep(name, self.tuning);
        self.write_presets();
        self.choose_preset(at);
        self.browser.pop();
        self.browser.set_at(at);
        self.tell(Notice::Saved);
    }

    /// Deletes one's own preset `i`; the one chosen, it plays through none.
    pub(super) fn delete_preset(&mut self, i: usize) {
        if !self.equalizer.delete(i) {
            return;
        }
        self.write_presets();
        match self.settings.eq {
            eq if eq == i => self.choose_preset(0),
            eq if eq > i => self.settings.eq -= 1,
            _ => {}
        }
        self.tell(Notice::Deleted);
    }

    /// The gains heard: New Preset's while it is open, else the preset chosen's.
    pub(super) fn heard_gains(&self) -> Gains {
        if self.browser.view() == View::NewPreset {
            self.tuning
        } else {
            self.equalizer.gains(self.settings.eq)
        }
    }

    fn write_presets(&mut self) {
        let text = self.equalizer.encode();
        self.out.push(Effect::Write(EQ_FILE.into(), text));
    }
}
