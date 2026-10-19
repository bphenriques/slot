//! The controller's buttons as they go down and up: read into keys, the pocket combo and
//! SELECT's skips among them, then each key acting as the screen shown, or the dark, has it.

use super::controller::{Input, Read};
use super::Music;
use crate::browse::keys::Mode;
use crate::browse::{Key, View};
use crate::screens;

impl Music {
    /// A button went down or up; true when it leaves the music screen. Some keys wait to see
    /// whether they are held, some act as they are let go, and some repeat.
    pub fn input(&mut self, input: Input) -> bool {
        self.stamp();
        // Blank, a button lights the screen again, and acts as it would.
        if matches!(input, Input::Down(_)) {
            self.blank = false;
        }
        // The pocket combo is for a pocket: Now Playing, or the lid closed; elsewhere the
        // shoulders are a list's.
        let pocket = self.settings.pocket && (self.dark || self.browser.view() == View::NowPlaying);
        let read = self.controller.read(input, pocket);
        // With Detailed Log on, each button as it came and as it was read, to see the device's
        // own order and timing.
        if log::log_enabled!(log::Level::Info) {
            let gap = self
                .last_input
                .map(|at| self.now.at.saturating_duration_since(at));
            log::info!("{input:?} read as {read:?}, {gap:?} after the last");
        }
        self.last_input = Some(self.now.at);
        let leaves = match read {
            Some(Read::Press(key)) => self.press(key),
            Some(Read::Release(key)) => self.release(key),
            None => false,
        };
        self.run();
        leaves
    }

    /// Every button let go, as the music screen is left: none of them does anything more.
    pub fn let_go(&mut self) {
        self.controller.forget();
        self.keys.cancel();
        self.blank = false;
    }

    fn press(&mut self, key: Key) -> bool {
        // The second of the pocket combo makes the two one, the first's doing forgotten.
        if key == Key::Pocket {
            self.keys.cancel();
            self.keys.press(Key::Pocket, Mode::Hold, self.now.at);
            return false;
        }
        // Locked, only the combo acts.
        if self.pocket.locked {
            return false;
        }
        let mode = if self.dark {
            let book = self.context().book.is_some();
            screens::binds::mode(&screens::binds::dark(book), key)
        } else {
            self.browser.mode(&self.context(), key)
        };
        self.keys
            .press(key, mode, self.now.at)
            .is_some_and(|press| self.act(press))
    }

    /// Either key of the combo let go ends it, tapped if it was not held.
    fn release(&mut self, key: Key) -> bool {
        let press = self
            .keys
            .release(Key::Pocket)
            .or_else(|| self.keys.release(key));
        press.is_some_and(|press| self.act(press))
    }

    /// How strong the motor should be, when that changes.
    pub fn rumble(&mut self) -> Option<u16> {
        self.stamp();
        self.pocket.rumble(self.now.at)
    }

    /// The screen went off or on: off, only the shoulders act, as a closed lid leaves them.
    pub fn set_dark(&mut self, dark: bool) {
        self.stamp();
        self.dark = dark;
        self.blank = false;
        self.let_go();
        // A lock is for the pocket: the lid opened, the keys are back.
        if !dark {
            self.pocket.locked = false;
        }
    }
}
