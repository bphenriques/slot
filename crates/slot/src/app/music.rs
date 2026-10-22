//! The music screen's side of `App`, apart from app.rs so upstream merges stay cheap.

#![warn(clippy::match_bool, clippy::undocumented_unsafe_blocks)]

use std::time::Duration;

use slot_gfx::Draw;
use slot_input::{Action, Btn, Millis, RawEvent};
use slot_music::{AlbumId, Art, Button, Input, Music, Screen};
use slot_music_ui::MusicTheme;
use slot_store::Shader;
use slot_ui::QuickRow;

use super::{App, Phase};

/// The music, playing through the device's own player: the one place a device is chosen for it.
pub(super) fn start(root: &std::path::Path) -> Music {
    use slot_music_ui::{ART_BOX, THUMB_PX};
    Music::spawn(root, ART_BOX, THUMB_PX, slot_music::Handle::slot)
}

impl App {
    pub(super) fn open_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.open();
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
        // Music playing in the dark postpones the screen-off power-off until it stops, and tells
        // the lid watcher (`card/System/lid.sh`) slot is busy, not hung, so it waits too.
        if matches!(self.phase, Phase::Doze { .. }) && self.music_audible() {
            self.dozed_at = self.now();
            if busy_due(self.music_frames.busy_at, self.now()) {
                self.music_frames.busy_at = Some(self.now());
                say_busy();
            }
        }
    }

    /// The music screen.
    pub(super) fn draw_music(&self, out: &mut Vec<slot_ui::Draw>) {
        out.extend(self.music_faces.iter().copied());
    }

    /// Powering off: what the music keeps on the card is written first.
    pub(super) fn flush_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.flush();
        }
    }

    /// A game is starting and takes the sound; the queue waits, paused.
    pub(super) fn pause_music(&mut self) {
        if let Some(music) = &mut self.music {
            music.pause();
        }
    }

    /// The screen is going off: the music says whether it plays on, and its screen returns on
    /// wake.
    pub(super) fn doze_music(&mut self) {
        self.music_on_wake = matches!(self.phase, Phase::Music);
        if let Some(music) = &mut self.music {
            music.set_dark(true);
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
        // Every button passes here, the music's or not: those down are kept, as a button held
        // is timed frame by frame. The lid is no button.
        let down = &mut self.music_frames.down;
        match ev {
            RawEvent::Down(btn) if btn != Btn::Lid && !down.contains(&btn) => down.push(btn),
            RawEvent::Up(btn) => down.retain(|&d| d != btn),
            RawEvent::Down(_) => {}
        }
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

    /// The screen off, by POWER or the lid: nothing shows until it is back, though music may
    /// play on for hours.
    pub fn resting(&self) -> bool {
        matches!(self.phase, Phase::Doze { .. })
    }

    /// Whether the next frame would show what the last one did, so need not be drawn, the panel
    /// keeping that one up: the screen off, or the music screen with nothing new to show and
    /// nothing on its way.
    fn holds_still(&self) -> bool {
        match &self.music_frames.drawn {
            Some(Drawn::Music { shown, wake }) if self.music_shown() => {
                let began = std::time::Instant::now();
                let still = still(&**shown, *wake, &self.music_now(), self.now());
                if let Some(music) = &self.music {
                    music.note_compare(began.elapsed());
                }
                still
            }
            // The screen off: drawn once so, and then nothing until it is back.
            Some(Drawn::Rest) => self.resting(),
            _ => false,
        }
    }

    /// What the next frame needs: whether it would show what the last one did, so need not be
    /// drawn; and, so, for how long the loop may rest until a button or the lid: until the
    /// soonest of the glue's next step and the music's next change, if any, and not at all while
    /// a button is down or something moves frame by frame.
    pub fn next_frame(&self) -> NextFrame {
        let still = self.holds_still();
        NextFrame {
            still,
            rest: still.then(|| self.rest_for()).flatten(),
        }
    }

    /// For how long the loop may rest once the frame up stands, as one just drawn does: none
    /// unless a frame of slot's own is drawn so, the music's or the screen off.
    pub fn rest_for(&self) -> Option<Duration> {
        let frames = &self.music_frames;
        let step = match frames.drawn.as_ref()? {
            Drawn::Music { wake: Some(at), .. } => Some(at.saturating_sub(self.now())),
            _ => None,
        };
        // A millisecond past the step: the clock counts whole milliseconds, and may read one short.
        let step = step.map(|ms| Duration::from_millis(ms + 1));
        let due = self.music.as_ref().and_then(Music::due_in);
        let soonest = step.into_iter().chain(due).min();
        let rests = frames.down.is_empty() && soonest != Some(Duration::ZERO);
        rests.then(|| soonest.unwrap_or(Duration::MAX))
    }

    /// The music screen was drawn, to move of itself next at `wake`, if it does.
    pub(crate) fn music_drawn(&mut self, wake: Option<Millis>) {
        let shown = Box::new(self.music_now());
        self.music_frames.drawn = Some(Drawn::Music { shown, wake });
    }

    /// A frame was drawn without the music screen: coming back to it draws it afresh.
    pub(crate) fn music_hidden(&mut self) {
        self.music_frames.drawn = self.resting().then_some(Drawn::Rest);
    }

    /// What the music screen shows now, but for what moves of itself.
    fn music_now(&self) -> Shown {
        let mut draws = Vec::new();
        self.draw(&mut draws);
        Shown {
            screen: self.music_screen(),
            art: self.music_art().map(|art| art.album),
            theme: self.music_theme(),
            draws,
            look: (
                self.shows_game(),
                self.blue_light(),
                self.screen_shader(),
                [
                    self.screen_power(),
                    self.screen_shake(),
                    self.overlay_slide(),
                ],
            ),
        }
    }

    /// Whether the music reads the buttons: on its screen; and with the screen off, only while
    /// it plays on (Screen Off set to play) and was in use, its screen open when the screen went
    /// off or still sounding. A screen gone off over a game or the shelf leaves the keys alone, so a
    /// shoulder pressed in a pocket never starts the music behind a game.
    pub fn music_takes_keys(&self) -> bool {
        let plays_on = self.music.as_ref().is_some_and(Music::plays_screen_off);
        match self.phase {
            Phase::Music => true,
            Phase::Doze { .. } => plays_on && (self.music_on_wake || self.music_audible()),
            _ => false,
        }
    }

    pub fn music_art(&self) -> Option<&Art> {
        self.music.as_ref()?.art()
    }

    /// The thumbnails the shown list's rows start with, as read.
    pub fn music_thumbs(&self) -> &[Art] {
        self.music.as_ref().map_or(&[], Music::thumbs)
    }

    /// The music screens' colours: the Theme setting, which may follow slot's appearance.
    pub fn music_theme(&self) -> MusicTheme {
        MusicTheme::of(self.light())
    }

    /// What the music screen draws, as slot draws it.
    pub fn set_music_draws(&mut self, draws: Vec<Draw>) {
        self.music_faces = draws;
    }

    /// For the performance log: how long the music screen took to draw.
    pub fn note_music_draw(&self, took: std::time::Duration) {
        if let Some(music) = &self.music {
            music.note_draw(took);
        }
    }

    /// Whether the music screen is the one shown.
    pub fn music_shown(&self) -> bool {
        matches!(self.phase, Phase::Music)
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

/// What the music screen shows, but for what moves of itself: what slot-music says, the picture
/// and colours its faces are drawn from, the draws (which hold anything of slot's over the music,
/// as its volume), and the settings the compositor draws them through. A frame showing the
/// same as the last needs no drawing.
#[derive(PartialEq, Debug)]
pub(super) struct Shown {
    screen: Option<Screen>,
    art: Option<AlbumId>,
    theme: MusicTheme,
    draws: Vec<Draw>,
    look: (bool, u8, Shader, [f32; 3]),
}

/// What the next frame needs, as `App::next_frame` says: whether it holds still, and for how
/// long at most the loop may rest; none to keep the frames' pace.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct NextFrame {
    pub still: bool,
    pub rest: Option<Duration>,
}

/// What tells whether the next frame is needed, and whether the loop may rest until a button:
/// what was last drawn where it can stay up, and the buttons down, slot's or the music's.
#[derive(Default)]
pub(super) struct Frames {
    drawn: Option<Drawn>,
    down: Vec<Btn>,
    /// When the lid watcher was last told slot is busy with music.
    busy_at: Option<Millis>,
}

/// How often the lid watcher is told slot is busy with music: well inside the time it trusts
/// that for (`BUSY_FOR` in `card/System/lid.sh`, 30 s).
const BUSY_EVERY: Millis = 10_000;
/// Where the lid watcher reads it: the seconds since boot, as it counts them.
const BUSY_FILE: &str = "/run/slot-busy";

/// Whether the lid watcher is due to be told again, last told at `at`.
fn busy_due(at: Option<Millis>, now: Millis) -> bool {
    at.is_none_or(|at| now.saturating_sub(at) >= BUSY_EVERY)
}

/// Tells the lid watcher slot is busy now; where there is no watcher, nothing is lost.
fn say_busy() {
    let up = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    if let Some(secs) = up.split(['.', ' ']).next().filter(|s| !s.is_empty()) {
        let _ = std::fs::write(BUSY_FILE, secs);
    }
}

/// What was last drawn, where it can stay up: the music screen, and when it next moves of
/// itself, if it does; or the screen off.
pub(super) enum Drawn {
    Music {
        shown: Box<Shown>,
        wake: Option<Millis>,
    },
    Rest,
}

/// Whether what was drawn, `then`, still stands `now`: it shows what would be drawn, and
/// nothing of it is due to move.
fn still<T: PartialEq>(then: &T, wake: Option<Millis>, shown: &T, now: Millis) -> bool {
    wake.is_none_or(|at| now < at) && then == shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lid_watcher_is_told_slot_is_busy_now_and_then_never_every_frame() {
        assert!(busy_due(None, 5), "the first time at once");
        assert!(
            !busy_due(Some(1_000), 10_999),
            "not again within ten seconds"
        );
        assert!(busy_due(Some(1_000), 11_000), "then again");
    }

    #[test]
    fn with_the_screen_off_over_a_game_the_music_leaves_the_keys_alone() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(
            d.path().join("Config/music.txt"),
            "screen_off = play\nhelp = seen\n",
        )
        .unwrap();
        let mut app = App::new(Vec::new());
        app.music = Some(start(d.path()));
        app.phase = Phase::Doze { cart: None };
        assert!(
            !app.music_takes_keys(),
            "gone off over a game, the music paused: not its keys"
        );
        app.music_on_wake = true;
        assert!(
            app.music_takes_keys(),
            "gone off on the music screen: the shoulders still play"
        );
        app.phase = Phase::Music;
        assert!(app.music_takes_keys());
    }

    #[test]
    fn what_was_drawn_stands_while_it_shows_what_would_be_and_nothing_is_due() {
        assert!(still(&1, None, &1, 50), "the same, nothing moving");
        assert!(!still(&1, None, &2, 50), "something new");
        assert!(still(&1, Some(60), &1, 50), "a step not yet due");
        assert!(!still(&1, Some(60), &1, 60), "due");
    }

    #[test]
    fn the_music_screen_holds_still_until_it_would_show_something_new() {
        let d = tempfile::tempdir().unwrap();
        let mut app = App::new(Vec::new());
        app.music = Some(start(d.path()));
        app.phase = Phase::Music;
        assert!(!app.holds_still(), "not yet drawn");
        app.music_drawn(None);
        assert!(app.holds_still(), "drawn, and nothing new");
        app.music_drawn(Some(app.now()));
        assert!(!app.holds_still(), "a scrolling line due to move");
        app.music_drawn(None);
        app.music_button(RawEvent::Down(Btn::Down));
        app.music_button(RawEvent::Up(Btn::Down));
        assert!(!app.holds_still(), "a key moved the highlight");
        // Away and back: drawn afresh, though it shows what it last did.
        app.music_drawn(None);
        app.phase = Phase::Shelf;
        assert!(!app.holds_still(), "slot draws every frame");
        app.music_hidden();
        app.phase = Phase::Music;
        assert!(!app.holds_still());
    }

    #[test]
    fn game_buttons_are_the_music_s_while_it_takes_the_keys_and_slot_keeps_its_own() {
        let d = tempfile::tempdir().unwrap();
        let mut app = App::new(Vec::new());
        app.music = Some(start(d.path()));
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
    fn the_loop_rests_held_still_until_its_next_step_and_never_with_a_button_down() {
        let d = tempfile::tempdir().unwrap();
        let mut app = App::new(Vec::new());
        app.music = Some(start(d.path()));
        app.phase = Phase::Music;
        let next = |app: &App| app.next_frame();
        assert_eq!(
            next(&app),
            NextFrame {
                still: false,
                rest: None
            },
            "not yet drawn"
        );
        app.music_drawn(None);
        assert_eq!(
            next(&app),
            NextFrame {
                still: true,
                rest: Some(Duration::MAX)
            },
            "nothing playing"
        );
        app.music_drawn(Some(app.now() + 40));
        assert_eq!(
            next(&app),
            NextFrame {
                still: true,
                rest: Some(Duration::from_millis(41))
            },
            "until a millisecond past a line's step"
        );
        // Just drawn, the frame stands until its next step, without a frame run first to see so.
        assert_eq!(app.rest_for(), Some(Duration::from_millis(41)));
        app.music_drawn(None);
        // A button held, slot's as much as the music's, is timed frame by frame.
        app.music_button(RawEvent::Down(Btn::VolUp));
        assert_eq!(next(&app).rest, None, "the volume held");
        app.music_button(RawEvent::Up(Btn::VolUp));
        assert_eq!(next(&app).rest, Some(Duration::MAX), "let go");
        // A frame of slot's own screens is drawn every frame, as ever.
        app.phase = Phase::Shelf;
        app.music_hidden();
        assert_eq!(app.rest_for(), None, "slot's own");
        // The lid, shut, is no button held.
        app.phase = Phase::Doze { cart: None };
        app.music_button(RawEvent::Down(Btn::Lid));
        app.music_hidden();
        assert_eq!(
            next(&app),
            NextFrame {
                still: true,
                rest: Some(Duration::MAX)
            },
            "the lid closed"
        );
    }

    #[test]
    fn the_lid_closed_is_drawn_once_and_then_held_until_it_opens() {
        let mut app = App::new(Vec::new());
        app.music_hidden();
        assert!(!app.holds_still(), "slot draws every frame");
        app.phase = Phase::Doze { cart: None };
        assert!(!app.holds_still(), "the lid just closed, not yet drawn so");
        app.music_hidden();
        assert!(app.holds_still(), "drawn closed, it stays up");
        app.phase = Phase::Shelf;
        assert!(!app.holds_still(), "opened");
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
