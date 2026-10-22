//! The controller's buttons as they go down and up: read into keys, the pocket combo and
//! SELECT's skips among them, then each key acting as the screen shown, or the dark, has it.

use super::controller::{Controller, Input, Read};
use super::pocket::PocketLock;
use super::Music;
use crate::browse::keys::{Keys, Mode};
use crate::browse::{Key, View};
use crate::screens;
use crate::settings::ScreenOff;
use std::time::Instant;

/// The buttons as the music reads them: those down, the key acting (tapped, held or
/// repeating), the pocket lock, and when the last came, for the log's trace of them.
#[derive(Default)]
pub(crate) struct Buttons {
    controller: Controller,
    pub(crate) keys: Keys<Key>,
    pub(crate) pocket: PocketLock,
    last_input: Option<Instant>,
}

impl Music {
    /// A button went down or up; true when it leaves the music screen. Some keys wait to see
    /// whether they are held, some act as they are let go, and some repeat.
    pub fn input(&mut self, input: Input) -> bool {
        self.stamp();
        // The pocket combo is for a pocket: Now Playing, or the screen off; elsewhere the
        // shoulders are a list's.
        let pocket = self.dark || self.browser.view() == View::NowPlaying;
        let read = self.buttons.controller.read(input, pocket);
        // With Detailed Log on, each button as it came and as it was read, to see the device's
        // own order and timing.
        if log::log_enabled!(log::Level::Info) {
            let gap = self
                .buttons
                .last_input
                .map(|at| self.now.at.saturating_duration_since(at));
            log::info!("{input:?} read as {read:?}, {gap:?} after the last");
        }
        self.buttons.last_input = Some(self.now.at);
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
        self.buttons.controller.forget();
        self.buttons.keys.cancel();
    }

    fn press(&mut self, key: Key) -> bool {
        // The second of the pocket combo makes the two one, the first's doing forgotten.
        if key == Key::Pocket {
            self.buttons.keys.cancel();
            self.buttons
                .keys
                .press(Key::Pocket, Mode::Hold, self.now.at);
            return false;
        }
        // Locked, only the combo acts.
        if self.buttons.pocket.locked {
            return false;
        }
        let mode = if self.dark {
            let book = self.context().book.is_some();
            screens::binds::mode(&screens::binds::dark(book), key)
        } else {
            self.browser.mode(&self.context(), key)
        };
        self.buttons
            .keys
            .press(key, mode, self.now.at)
            .is_some_and(|press| self.act(press))
    }

    /// Either key of the combo let go ends it, tapped if it was not held.
    fn release(&mut self, key: Key) -> bool {
        let press = self
            .buttons
            .keys
            .release(Key::Pocket)
            .or_else(|| self.buttons.keys.release(key));
        press.is_some_and(|press| self.act(press))
    }

    /// How strong the motor should be, when that changes.
    pub fn rumble(&mut self) -> Option<u16> {
        self.stamp();
        self.buttons.pocket.rumble(self.now.at)
    }

    /// The screen went off or on: off, the music pauses unless set to play on, and only the
    /// shoulders act, as a pocket or a closed lid leaves them.
    pub fn set_dark(&mut self, dark: bool) {
        self.stamp();
        self.dark = dark;
        self.let_go();
        // A lock is for the pocket: set to, the screen going off locks the keys, quietly, as its
        // going off says it already; back on, the keys are back.
        let locks = self.settings.screen_off == ScreenOff::PlayLocked;
        self.buttons.pocket.locked = dark && (locks || self.buttons.pocket.locked);
        if dark && !self.settings.screen_off.plays() {
            self.pause();
        }
    }
}
