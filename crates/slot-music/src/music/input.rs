//! Keys as they go down and up: the pocket combo first, then the key as the screen shown, or
//! the dark, has it act.

use super::Music;
use crate::browse::keys::Mode;
use crate::browse::Key;
use crate::screens;

impl Music {
    /// `key` went down; true when it leaves the music screen. Some keys wait to see whether
    /// they are held, and some repeat.
    pub fn press(&mut self, key: Key) -> bool {
        // The second key of the pocket combo makes the two one, the first's doing forgotten.
        if self.pocket.press(self.settings.pocket, key) {
            self.keys.cancel();
            self.keys.press(Key::Pocket, Mode::Hold);
            return false;
        }
        // Locked, only the combo acts.
        if self.pocket.locked {
            return false;
        }
        let mode = match self.dark {
            true => screens::dark::mode(key),
            false => self.browser.mode(&self.context(), key),
        };
        let leaves = self
            .keys
            .press(key, mode)
            .is_some_and(|press| self.act(press));
        self.run();
        leaves
    }

    /// `key` went up, or with `None`, every key, as the music screen is left; true when a tap
    /// leaves the music screen.
    pub fn release(&mut self, key: Option<Key>) -> bool {
        let Some(key) = key else {
            self.pocket.forget();
            self.keys.cancel();
            return false;
        };
        self.pocket.release(key);
        // Either key of the combo let go ends it, tapped if it was not held.
        let press = self
            .keys
            .release(Key::Pocket)
            .or_else(|| self.keys.release(key));
        let leaves = press.is_some_and(|press| self.act(press));
        self.run();
        leaves
    }

    /// How strong the motor should be, when that changes.
    pub fn rumble(&mut self) -> Option<u16> {
        self.pocket.rumble()
    }

    /// The screen went off or on: off, only the shoulders act, as a closed lid leaves them.
    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
        self.keys.cancel();
        self.pocket.forget();
        // A lock is for the pocket: the lid opened, the keys are back.
        if !dark {
            self.pocket.locked = false;
        }
    }
}
