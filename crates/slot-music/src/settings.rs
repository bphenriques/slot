use slot_player::Leveling;

use crate::playback::player::Repeat;

/// The Music Settings choices, and the last repeat mode.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    pub repeat: Repeat,
    /// `screen_off = play` keeps music going with the screen off or the lid shut; `pause`
    /// (default) pauses it.
    pub play_screen_off: bool,
    /// The size of library lists and the queue; menus keep slot's size.
    pub list_size: ListSize,
    /// Show the clock and battery on the music screens (`clock = on`; off by default, and only
    /// in `music.txt`).
    pub clock: bool,
    /// Show the next track under Now Playing (`up_next`; only in `music.txt`).
    pub up_next: bool,
    /// Tint Now Playing, its background and progress bar, from the album picture.
    pub color: bool,
    /// The music screens' colours.
    pub theme: Theme,
    /// Seconds Left and Right jump in an audiobook.
    pub jump: u32,
    /// Both sides alike (`mono = on`), for one ear or one speaker; slot's player only.
    pub mono: bool,
    /// Songs brought to one loudness from their ReplayGain (`leveling = on`): an album's as a
    /// whole when played in order, each song's when shuffled; slot's player only.
    pub leveling: bool,
    /// Seconds a song's end fades into the next (`crossfade = 0`, 3 or 6); never a book's.
    pub crossfade: u32,
    /// For books: pauses cut short (`skip_silence = on`), and dB louder (`boost = 0`, 3 or 6).
    pub skip_silence: bool,
    pub boost: u32,
    /// Whether L1 + R1 or L2 + R2 together, tapped, play or pause and, held, lock the music's
    /// keys, as in a pocket (`pocket = on`, the default); and whether a lock buzzes
    /// (`pocket_buzz = on`).
    pub pocket: bool,
    pub pocket_buzz: bool,
    /// How long each buzz lasts, in milliseconds (`buzz = 250`), on the Debug page, to find
    /// what the motor is felt at.
    pub buzz: u32,
    /// Whether the performance log runs (`perf = on`), on the Debug page.
    pub perf: bool,
}

/// A setting the music screens change. The sleep timer is on Now Playing's menu and never
/// saved; the rest are on the Music Settings page. Clock and Up Next are only in `music.txt`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Setting {
    ScreenOff,
    Pocket,
    PocketBuzz,
    Jump,
    Mono,
    Crossfade,
    Leveling,
    SkipSilence,
    Boost,
    ListSize,
    /// On Now Playing's menu, as the sleep timer is, but kept.
    Repeat,
    SleepTimer,
    Color,
    Theme,
    /// On the Debug page.
    BuzzLength,
    PerfLog,
}

impl Setting {
    /// In page order: listening first, then looks.
    /// Music Settings' own: listening, then looks, the Sound and Audiobooks pages between.
    pub const ALL: [Setting; 6] = [
        Setting::ScreenOff,
        Setting::Pocket,
        Setting::PocketBuzz,
        Setting::ListSize,
        Setting::Color,
        Setting::Theme,
    ];

    /// The Sound page's, slot's player's alone. Songs always follow on with no gap, as on
    /// Apple Music: nothing is lost by it.
    pub const SOUND: [Setting; 3] = [Setting::Crossfade, Setting::Leveling, Setting::Mono];

    /// The Audiobooks page's: the jump, then what slot's player alone does for a voice.
    pub const BOOKS: [Setting; 3] = [Setting::Jump, Setting::SkipSilence, Setting::Boost];
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ListSize {
    Small,
    Normal,
    Large,
}

impl ListSize {
    pub(crate) const ORDER: [ListSize; 3] = [ListSize::Small, ListSize::Normal, ListSize::Large];

    /// Rows per page, set around Normal. The type size follows the row spacing (slot-ui).
    pub fn page(self) -> usize {
        match self {
            ListSize::Small => 10,
            ListSize::Normal => 8,
            ListSize::Large => 7,
        }
    }

    /// Its name in `music.txt`, never shown: the screens word it through `text`.
    pub(crate) fn key(self) -> &'static str {
        match self {
            ListSize::Small => "small",
            ListSize::Normal => "normal",
            ListSize::Large => "large",
        }
    }
}

/// The jumps an audiobook offers, as players do: 15 is Apple's, 30 Audible's.
pub(crate) const JUMPS: [u32; 3] = [10, 15, 30];

/// The music screens' colours: slot's appearance, or always dark or light.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Theme {
    Slot,
    Dark,
    Light,
}

impl Theme {
    pub(crate) const ORDER: [Theme; 3] = [Theme::Slot, Theme::Dark, Theme::Light];

    /// Whether the screens are light, when slot's appearance is `light` or not.
    pub fn light(self, light: bool) -> bool {
        match self {
            Theme::Slot => light,
            Theme::Dark => false,
            Theme::Light => true,
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Theme::Slot => "slot",
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

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
            play_screen_off: false,
            list_size: ListSize::Normal,
            clock: false,
            up_next: true,
            color: true,
            theme: Theme::Slot,
            jump: 30,
            mono: false,
            leveling: false,
            crossfade: 0,
            skip_silence: false,
            boost: 0,
            pocket: true,
            pocket_buzz: true,
            buzz: 250,
            perf: false,
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
            Setting::ScreenOff => self.play_screen_off ^= true,
            Setting::ListSize => self.list_size = cycle(&ListSize::ORDER, self.list_size, by),
            Setting::Color => self.color ^= true,
            Setting::Mono => self.mono ^= true,
            Setting::Crossfade => self.crossfade = cycle(&FADES, self.crossfade, by),
            Setting::SkipSilence => self.skip_silence ^= true,
            Setting::Boost => self.boost = cycle(&BOOSTS, self.boost, by),
            Setting::Pocket => self.pocket ^= true,
            Setting::PocketBuzz => self.pocket_buzz ^= true,
            Setting::BuzzLength => self.buzz = cycle(&BUZZES, self.buzz, by),
            Setting::PerfLog => self.perf ^= true,
            Setting::Leveling => self.leveling ^= true,
            Setting::Theme => self.theme = cycle(&Theme::ORDER, self.theme, by),
            Setting::Jump => self.jump = cycle(&JUMPS, self.jump, by),
            Setting::Repeat => self.repeat = cycle(&Repeat::ORDER, self.repeat, by),
            Setting::SleepTimer => {}
        }
    }
}

/// Crossfade's seconds and Volume Boost's dB, in the order A goes round them.
pub const FADES: [u32; 3] = [0, 3, 6];
/// The buzz lengths the Debug page offers, in milliseconds.
pub const BUZZES: [u32; 4] = [120, 250, 400, 600];
pub const BOOSTS: [u32; 3] = [0, 3, 6];
