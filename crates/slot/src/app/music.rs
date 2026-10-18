//! The music screen's side of `App`, apart from app.rs so upstream merges stay cheap.

use slot_input::{Action, Btn, RawEvent};
use slot_music::{Art, Button, Input, Music, Screen, Theme};
use slot_ui::{MusicFaces, MusicTheme, QuickRow};

use super::{App, Phase};

impl App {
    pub(super) fn open_music(&mut self) {
        if self.music.is_some() {
            self.phase = Phase::Music;
        }
    }

    /// slot's own keys on the music screen: MENU tapped leaves it for slot's menu. The rest
    /// came as buttons, `music_button`'s.
    pub(super) fn music_input(&mut self, action: Action) {
        if action == Action::QuickMenu {
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
        // Off-screen keys let go, but not in the dark, where the shoulders still play.
        let dark = matches!(self.phase, Phase::Doze { .. });
        let mut rumble = None;
        if let Some(music) = &mut self.music {
            if !shown && !dark {
                music.let_go();
            }
            music.poll(game);
            rumble = music.rumble();
        }
        if let Some(strength) = rumble {
            self.set_rumble(strength);
        }
        // Music playing in the dark postpones the screen-off power-off until it stops.
        if matches!(self.phase, Phase::Doze { .. }) && self.music_audible() {
            self.dozed_at = self.now();
        }
    }

    /// A game is starting and takes the sound; the queue waits, paused.
    /// The music screen, with slot's clock and battery when its header shows them.
    pub(super) fn draw_music(&self, out: &mut Vec<slot_ui::Draw>) {
        slot_ui::draw_music(&self.music_faces, out);
        if self.music_faces.clock {
            let (battery, percent) = (self.battery, self.battery_percent);
            let (bolt, clock) = (self.bolt, self.shelf_clock);
            slot_ui::draw_music_clock(&self.music_faces, battery, percent, bolt, clock, out);
        }
    }

    /// Powering off: what the music keeps on the card is written first.
    pub(super) fn flush_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.flush();
        }
    }

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

    /// A game button, raw, while music takes the keys: the music's own, read as its controller
    /// has them, its chords and combos its own too, ahead of slot's gestures, which would read
    /// the triggers as rewind and SELECT as theirs. True when it took it; MENU, power, the
    /// volume and the lid stay slot's.
    pub fn music_button(&mut self, ev: RawEvent) -> bool {
        let input = match ev {
            RawEvent::Down(btn) => button(btn).map(Input::Down),
            RawEvent::Up(btn) => button(btn).map(Input::Up),
        };
        let (Some(input), true) = (input, self.music_takes_keys()) else {
            return false;
        };
        if self.music.as_mut().is_some_and(|m| m.input(input)) {
            self.leave_music();
        }
        true
    }

    /// The lid closed: nothing shows until it opens, though music may play on for hours.
    pub fn resting(&self) -> bool {
        matches!(self.phase, Phase::Doze { .. })
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

    /// For the performance log: how long the music screen took to draw.
    pub fn note_music_draw(&self, took: std::time::Duration) {
        if let Some(music) = &self.music {
            music.note_draw(took);
        }
    }

    pub fn music_owns_audio(&self) -> bool {
        self.music.as_ref().is_some_and(Music::owns_audio)
    }

    pub fn music_audible(&self) -> bool {
        self.music.as_ref().is_some_and(Music::audible)
    }

    /// The codec's level while music may sound: slot's volume, scaled through fades.
    pub fn music_volume(&self) -> Option<u8> {
        let music = self.music.as_ref().filter(|m| m.audible())?;
        Some((f32::from(self.output_volume()) * music.volume_scale()).round() as u8)
    }
}

/// The music's button for one of the device's, if it takes it.
fn button(btn: Btn) -> Option<Button> {
    Some(match btn {
        Btn::Up => Button::Up,
        Btn::Down => Button::Down,
        Btn::Left => Button::Left,
        Btn::Right => Button::Right,
        Btn::L1 => Button::L1,
        Btn::R1 => Button::R1,
        Btn::L2 => Button::L2,
        Btn::R2 => Button::R2,
        Btn::A => Button::A,
        Btn::B => Button::B,
        Btn::X => Button::X,
        Btn::Y => Button::Y,
        Btn::Start => Button::Start,
        Btn::Select => Button::Select,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_buttons_are_the_music_s_while_it_takes_the_keys_and_slot_keeps_its_own() {
        let d = tempfile::tempdir().unwrap();
        let mut app = App::new(Vec::new());
        app.music = Some(Music::spawn(d.path(), slot_ui::ART_BOX));
        let down = RawEvent::Down(Btn::R2);
        assert!(!app.music_button(down), "rewind and fast forward elsewhere");
        app.phase = Phase::Music;
        assert!(app.music_button(down));
        assert!(
            app.music_button(RawEvent::Up(Btn::Select)),
            "SELECT, for its chords"
        );
        for slot_s in [Btn::Menu, Btn::VolUp, Btn::Power, Btn::Lid] {
            assert!(
                !app.music_button(RawEvent::Down(slot_s)),
                "{slot_s:?} is slot's"
            );
        }
    }

    #[test]
    fn only_the_lid_closed_rests() {
        let mut app = App::new(Vec::new());
        assert!(!app.resting());
        app.phase = Phase::Doze { cart: None };
        assert!(app.resting());
    }

    #[test]
    fn both_triggers_held_reach_the_music_together_through_the_session_and_lock_it() {
        let d = tempfile::tempdir().unwrap();
        let sink = Box::new(crate::audio::StubSink::draining());
        let mut s = crate::session::Session::boot_with(d.path().to_path_buf(), sink);
        s.app_mut().phase = Phase::Music;
        // On Now Playing, where the combo is a pocket's, as SELECT takes it.
        s.feed([RawEvent::Down(Btn::Select), RawEvent::Up(Btn::Select)], 8);
        // As the triggers come off the device: one, then the other, held, then let go.
        s.feed([RawEvent::Down(Btn::L2)], 16);
        s.feed([RawEvent::Down(Btn::R2)], 32);
        let held = std::time::Instant::now();
        let mut now = 32;
        while held.elapsed() < std::time::Duration::from_millis(600) {
            now += 16;
            s.feed([], now);
            s.update(1.0 / 60.0);
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        s.feed([RawEvent::Up(Btn::R2), RawEvent::Up(Btn::L2)], now + 16);
        s.update(1.0 / 60.0);
        let corner = s.app().music_screen().map(|screen| screen.corner);
        assert_eq!(
            corner.as_deref(),
            Some("Locked"),
            "the pocket combo, not a seek"
        );
    }
}
