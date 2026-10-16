//! The music screen's side of `App`, apart from app.rs so upstream merges stay cheap.

use slot_input::{Action, Btn};
use slot_music::{Art, Key, Music, Screen, Theme};
use slot_ui::{MusicFaces, MusicTheme, QuickRow};

use super::{App, Phase};

impl App {
    pub(super) fn open_music(&mut self) {
        if self.music.is_some() {
            self.phase = Phase::Music;
        }
    }

    pub(super) fn music_input(&mut self, action: Action) {
        // L2 and R2 arrive as slot's rewind and fast forward.
        let up = match action {
            Action::GbaUp(btn) => key(btn),
            Action::RewindStop => Some(Key::L2),
            Action::FfStop => Some(Key::R2),
            _ => None,
        };
        // Keys that wait to see whether they are held act when let go: B leaves from the top.
        if let Some(key) = up {
            if self.music.as_mut().is_some_and(|m| m.release(Some(key))) {
                self.leave_music();
            }
            return;
        }
        let key = match action {
            Action::RewindStart => Key::L2,
            Action::FfStart => Key::R2,
            Action::GbaDown(btn) => match key(btn) {
                Some(key) => key,
                None => return,
            },
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
        let shown = matches!(self.phase, Phase::Music);
        if let Some(music) = &mut self.music {
            if !shown {
                music.release(None);
            }
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
            music.set_dark(true);
            if !music.plays_screen_off() {
                music.pause();
            }
        }
    }

    /// With the screen off, the keys a closed lid leaves: the shoulders, when music plays on.
    pub(super) fn music_dark_input(&mut self, action: Action) {
        let Some(music) = self.music.as_mut().filter(|m| m.plays_screen_off()) else {
            return;
        };
        let (key, down) = match action {
            Action::GbaDown(btn) => (key(btn), true),
            Action::GbaUp(btn) => (key(btn), false),
            Action::FfStart => (Some(Key::R2), true),
            Action::FfStop => (Some(Key::R2), false),
            _ => return,
        };
        match (key, down) {
            (Some(key), true) => {
                music.press(key);
            }
            (Some(key), false) => {
                music.release(Some(key));
            }
            (None, _) => {}
        }
    }

    pub(super) fn wake_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.set_dark(false);
        }
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

    /// Whether music takes the keys: shown, or playing on with the screen off.
    pub fn music_takes_keys(&self) -> bool {
        let dark = self.music.as_ref().is_some_and(Music::plays_screen_off);
        match self.phase {
            Phase::Music => true,
            Phase::Doze { .. } => dark,
            _ => false,
        }
    }

    pub fn music_art(&self) -> Option<&Art> {
        self.music.as_ref()?.art()
    }

    /// The music screens' colours: the Theme setting, which may follow slot's appearance.
    pub fn music_theme(&self) -> MusicTheme {
        let theme = self.music.as_ref().map_or(Theme::Slot, Music::theme);
        MusicTheme::of(theme.light(self.light()))
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

    /// The codec's level while Muse may sound: slot's volume, scaled through fades.
    pub fn music_volume(&self) -> Option<u8> {
        let music = self.music.as_ref().filter(|m| m.audible())?;
        Some((f32::from(self.output_volume()) * music.volume_scale()).round() as u8)
    }
}

/// The music screen's key for a button, if it has one.
fn key(btn: Btn) -> Option<Key> {
    Some(match btn {
        Btn::Up => Key::Up,
        Btn::Down => Key::Down,
        Btn::Left => Key::Left,
        Btn::Right => Key::Right,
        Btn::L1 => Key::L1,
        Btn::R1 => Key::R1,
        Btn::A => Key::A,
        Btn::B => Key::B,
        Btn::X => Key::X,
        Btn::Y => Key::Y,
        Btn::Start => Key::Start,
        Btn::Select => Key::Select,
        _ => return None,
    })
}
