//! The music screen's side of `App`, apart from app.rs so upstream merges stay cheap.

use slot_input::{Action, Btn};
use slot_music::{Art, Key, Music, Screen};
use slot_ui::{MusicFaces, QuickRow};

use super::{App, Phase};

impl App {
    pub(super) fn open_music(&mut self) {
        if self.music.is_some() {
            self.phase = Phase::Music;
        }
    }

    pub(super) fn music_input(&mut self, action: Action) {
        let key = match action {
            Action::GbaDown(Btn::Up) => Key::Up,
            Action::GbaDown(Btn::Down) => Key::Down,
            Action::GbaDown(Btn::Left) => Key::Left,
            Action::GbaDown(Btn::Right) => Key::Right,
            Action::GbaDown(Btn::L1) => Key::L1,
            Action::GbaDown(Btn::R1) => Key::R1,
            Action::GbaDown(Btn::A) => Key::A,
            Action::GbaDown(Btn::B) => Key::B,
            Action::GbaDown(Btn::X) => Key::X,
            Action::GbaDown(Btn::Y) => Key::Y,
            Action::GbaDown(Btn::Start) => Key::Start,
            Action::GbaDown(Btn::Select) => Key::Select,
            // SELECT with a shoulder: save and load state in a game, track skips here.
            Action::LoadState => Key::SelectL1,
            Action::SaveState => Key::SelectR1,
            Action::QuickMenu => return self.leave_music(),
            _ => return,
        };
        if self.music.as_mut().is_some_and(|m| m.press(key)) {
            self.leave_music();
        }
    }

    fn leave_music(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::Music,
        };
    }

    pub(super) fn poll_music(&mut self) {
        let game = matches!(
            self.phase,
            Phase::Inserting { .. }
                | Phase::Playing { .. }
                | Phase::Polaroids { .. }
                | Phase::Ejecting { .. }
        );
        if let Some(music) = &mut self.music {
            music.poll(game);
        }
        // Music playing in the dark postpones the screen-off power-off until it stops.
        if matches!(self.phase, Phase::Doze { .. }) && self.music_audible() {
            self.dozed_at = self.now();
        }
    }

    /// A game is starting and takes the sound; the queue waits, paused.
    pub(super) fn pause_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.pause();
        }
    }

    /// The screen is going off: music pauses unless `screen_off = play`, and the music screen
    /// returns on wake.
    pub(super) fn doze_music(&mut self) {
        self.music_on_wake = matches!(self.phase, Phase::Music);
        if let Some(music) = &mut self.music {
            if !music.plays_screen_off() {
                music.pause();
            }
        }
    }

    /// With the screen off, SELECT and a shoulder still skip tracks.
    pub(super) fn music_dark_input(&mut self, action: Action) {
        let key = match action {
            Action::LoadState => Key::SelectL1,
            Action::SaveState => Key::SelectR1,
            _ => return,
        };
        if let Some(music) = self.music.as_mut().filter(|m| m.plays_screen_off()) {
            music.press(key);
        }
    }

    pub(super) fn wake_music(&mut self) {
        if std::mem::take(&mut self.music_on_wake) && matches!(self.phase, Phase::Shelf) {
            self.phase = Phase::Music;
        }
    }

    pub fn music_screen(&self) -> Option<Screen> {
        match self.phase {
            Phase::Music => self.music.as_ref().map(Music::screen),
            _ => None,
        }
    }

    pub fn music_art(&self) -> Option<&Art> {
        self.music.as_ref()?.art()
    }

    pub fn set_music_faces(&mut self, faces: MusicFaces) {
        self.music_faces = faces;
    }

    pub fn music_owns_audio(&self) -> bool {
        self.music.as_ref().is_some_and(Music::owns_audio)
    }

    pub fn music_audible(&self) -> bool {
        self.music.as_ref().is_some_and(Music::audible)
    }
}
