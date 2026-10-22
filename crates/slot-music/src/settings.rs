use slot_player::Leveling;

use crate::playback::player::Repeat;

/// The Settings choices, and the last repeat mode.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    pub repeat: Repeat,
    /// What the screen going off or the lid shutting does to music playing (`screen_off`).
    pub screen_off: ScreenOff,
    /// Music opens on Now Playing while something plays (`open = playing`), as phones do; else
    /// where it was left (`open = left`).
    pub open_playing: bool,
    /// Seconds Left and Right jump in an audiobook.
    pub jump: u32,
    /// Both sides alike (`mono = on`), for one ear or one speaker; slot's player only.
    pub mono: bool,
    /// Now Playing's background moves with what is heard (`visualizer = on`), faintly.
    pub visualizer: bool,
    /// Songs brought to one loudness from their ReplayGain (`leveling = on`): an album's as a
    /// whole when played in order, each song's when shuffled; slot's player only.
    pub leveling: bool,
    /// Seconds a song's end fades into the next (`crossfade = 0`, 3 or 6); never a book's.
    pub crossfade: u32,
    /// The equalizer's preset, from 1 as `Config/music-eq.txt` lists them; 0 is off (`eq = <its
    /// name>`, kept by name, so a preset added or moved in the file stays the one chosen).
    pub eq: usize,
    /// For books: pauses cut short (`skip_silence = on`), and dB louder (`boost = 0`, 3 or 6).
    pub skip_silence: bool,
    pub boost: u32,
    /// Whether the performance log runs (`perf = on`), on the Advanced page.
    pub perf: bool,
    /// Whether songs listened to go into `.scrobbler.log` for a Last.fm uploader
    /// (`scrobbler = on`), on the Advanced page.
    pub scrobbler: bool,
    /// Whether `Logs/slot.log` tells what happens as well as what went wrong (`log = on`), on the
    /// Advanced page: for someone asked to send it.
    pub log: bool,
    /// Whether Help was shown on the first start (`help = seen`), not to show it again.
    pub help_seen: bool,
}

/// What the screen going off or the lid shutting does to music playing.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ScreenOff {
    /// It pauses (`pause`).
    #[default]
    Pause,
    /// It plays on, the shoulders still skipping and pausing (`play`).
    Play,
    /// It plays on, the keys locked until the lid opens, for a pocket (`locked`): as the two
    /// shoulders held do, without them.
    PlayLocked,
}

impl ScreenOff {
    pub const ALL: [ScreenOff; 3] = [ScreenOff::Pause, ScreenOff::Play, ScreenOff::PlayLocked];

    /// Whether music plays on.
    pub fn plays(self) -> bool {
        self != ScreenOff::Pause
    }
}

/// A setting the music screens change. The sleep timer is on Now Playing's menu and never
/// saved; the rest are on the Settings page.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Setting {
    ScreenOff,
    OpenPlaying,
    Jump,
    Mono,
    Visualizer,
    Equalizer,
    Crossfade,
    Leveling,
    SkipSilence,
    Boost,
    /// On Now Playing's menu, as the sleep timer is, but kept.
    Repeat,
    SleepTimer,
    /// On the Advanced page.
    PerfLog,
    Log,
    Scrobbler,
}

impl Setting {
    /// Settings' own, after its Music and Audiobooks pages.
    pub const ALL: [Setting; 2] = [Setting::ScreenOff, Setting::OpenPlaying];

    /// The Sound page's, slot's player's alone. Songs always follow on with no gap, as on
    /// Apple Music: nothing is lost by it.
    pub const MUSIC: [Setting; 4] = [
        Setting::Crossfade,
        Setting::Leveling,
        Setting::Mono,
        Setting::Visualizer,
    ];

    /// The Audiobooks page's: the jump, then what slot's player alone does for a voice.
    pub const BOOKS: [Setting; 2] = [Setting::Jump, Setting::Boost];

    /// Advanced's Experimental page: what is still being shaped, the equalizer and skipping a
    /// book's silences, which guesses at where speech pauses.
    pub const EXPERIMENTAL: [Setting; 2] = [Setting::Equalizer, Setting::SkipSilence];
}

/// The jumps an audiobook offers, as players do: 15 is Apple's, 30 Audible's.
pub(crate) const JUMPS: [u32; 3] = [10, 15, 30];

/// The choice `by` steps to from `at` in `order`: Left (-1) and Right (1) stop at the ends,
/// A (0) goes round.
pub fn cycle<T: Copy + PartialEq>(order: &[T], at: T, by: i32) -> T {
    let n = order.len() as i32;
    let i = order.iter().position(|&o| o == at).unwrap_or(0) as i32;
    let to = match by {
        0 => (i + 1) % n,
        by => (i + by).clamp(0, n - 1),
    };
    order[to as usize]
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            repeat: Repeat::Off,
            screen_off: ScreenOff::Pause,
            open_playing: false,
            jump: 30,
            mono: false,
            visualizer: false,
            leveling: false,
            crossfade: 0,
            eq: 0,
            skip_silence: false,
            boost: 0,
            perf: false,
            log: false,
            scrobbler: false,
            help_seen: false,
        }
    }
}

impl Settings {
    /// The leveling for a queue: an album's songs at its own loudness when played in order,
    /// each song's when shuffled, as foobar2000 chooses by the order.
    pub fn level(&self, shuffled: bool) -> Leveling {
        match (self.leveling, shuffled) {
            (false, _) => Leveling::Off,
            (true, false) => Leveling::Album,
            (true, true) => Leveling::Track,
        }
    }

    /// Left, Right or A on a setting. The sleep timer is the facade's, not a saved setting.
    pub fn change(&mut self, setting: Setting, by: i32) {
        match setting {
            Setting::ScreenOff => self.screen_off = cycle(&ScreenOff::ALL, self.screen_off, by),
            Setting::OpenPlaying => self.open_playing ^= true,
            Setting::Mono => self.mono ^= true,
            Setting::Visualizer => self.visualizer ^= true,
            Setting::Crossfade => self.crossfade = cycle(&FADES, self.crossfade, by),
            Setting::SkipSilence => self.skip_silence ^= true,
            Setting::Boost => self.boost = cycle(&BOOSTS, self.boost, by),
            Setting::PerfLog => self.perf ^= true,
            // Stepped by the music, which knows the presets.
            Setting::Equalizer => {}
            Setting::Log => self.log ^= true,
            Setting::Scrobbler => self.scrobbler ^= true,
            Setting::Leveling => self.leveling ^= true,
            Setting::Jump => self.jump = cycle(&JUMPS, self.jump, by),
            Setting::Repeat => self.repeat = cycle(&Repeat::ORDER, self.repeat, by),
            Setting::SleepTimer => {}
        }
    }
}

/// Crossfade's seconds and Volume Boost's dB, in the order A goes round them.
pub const FADES: [u32; 3] = [0, 3, 6];
pub const BOOSTS: [u32; 3] = [0, 3, 6];
