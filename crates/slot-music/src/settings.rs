use std::path::Path;

use slot_store::ini;

use crate::player::Repeat;

/// The music player's settings, apart from slot's to keep upstream merges cheap.
pub const SETTINGS_FILE: &str = "Config/music.txt";

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
}

/// A setting the music screens change. The sleep timer is on Now Playing's menu and never
/// saved; the rest are on the Music Settings page. Clock and Up Next are only in `music.txt`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Setting {
    ScreenOff,
    Jump,
    ListSize,
    /// On Now Playing's menu, as the sleep timer is, but kept.
    Repeat,
    SleepTimer,
    Color,
    Theme,
}

impl Setting {
    /// In page order: listening first, then looks.
    pub const ALL: [Setting; 5] = [
        Setting::ScreenOff,
        Setting::Jump,
        Setting::ListSize,
        Setting::Color,
        Setting::Theme,
    ];
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ListSize {
    Small,
    Normal,
    Large,
}

impl ListSize {
    const ORDER: [ListSize; 3] = [ListSize::Small, ListSize::Normal, ListSize::Large];

    /// Rows per page, set around Normal. The type size follows the row spacing (slot-ui).
    pub fn page(self) -> usize {
        match self {
            ListSize::Small => 10,
            ListSize::Normal => 8,
            ListSize::Large => 7,
        }
    }

    /// Its name in `music.txt`, never shown: the screens word it through `text`.
    fn key(self) -> &'static str {
        match self {
            ListSize::Small => "small",
            ListSize::Normal => "normal",
            ListSize::Large => "large",
        }
    }
}

/// The jumps an audiobook offers, as players do: 15 is Apple's, 30 Audible's.
const JUMPS: [u32; 3] = [10, 15, 30];

/// The music screens' colours: slot's appearance, or always dark or light.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Theme {
    Slot,
    Dark,
    Light,
}

impl Theme {
    const ORDER: [Theme; 3] = [Theme::Slot, Theme::Dark, Theme::Light];

    /// Whether the screens are light, when slot's appearance is `light` or not.
    pub fn light(self, light: bool) -> bool {
        match self {
            Theme::Slot => light,
            Theme::Dark => false,
            Theme::Light => true,
        }
    }

    fn key(self) -> &'static str {
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
        }
    }
}

impl Settings {
    /// Left, Right or A on a setting. The sleep timer is the facade's, not a saved setting.
    pub fn change(&mut self, setting: Setting, by: i32) {
        match setting {
            Setting::ScreenOff => self.play_screen_off ^= true,
            Setting::ListSize => self.list_size = cycle(&ListSize::ORDER, self.list_size, by),
            Setting::Color => self.color ^= true,
            Setting::Theme => self.theme = cycle(&Theme::ORDER, self.theme, by),
            Setting::Jump => self.jump = cycle(&JUMPS, self.jump, by),
            Setting::Repeat => self.repeat = cycle(&Repeat::ORDER, self.repeat, by),
            Setting::SleepTimer => {}
        }
    }

    pub fn read(root: &Path) -> Settings {
        let file = ini::read(root, SETTINGS_FILE);
        let get = |key: &str| file.get(key).map(String::as_str);
        let is = |key: &str, value: &str| get(key).is_some_and(|v| v.eq_ignore_ascii_case(value));
        let list = |v: &str| {
            ListSize::ORDER
                .into_iter()
                .find(|s| s.key().eq_ignore_ascii_case(v))
        };
        let theme = |v: &str| {
            Theme::ORDER
                .into_iter()
                .find(|t| t.key().eq_ignore_ascii_case(v))
        };
        Settings {
            repeat: get("repeat").and_then(Repeat::parse).unwrap_or_default(),
            play_screen_off: is("screen_off", "play"),
            list_size: get("list").and_then(list).unwrap_or(ListSize::Normal),
            clock: is("clock", "on"),
            up_next: !is("up_next", "off"),
            color: !is("color", "off"),
            theme: get("theme").and_then(theme).unwrap_or(Theme::Slot),
            jump: get("jump")
                .and_then(|v| v.parse().ok())
                .filter(|j| JUMPS.contains(j))
                .unwrap_or(30),
        }
    }

    pub fn write(&self, root: &Path) {
        let pick = |on: bool, yes: &'static str, no: &'static str| if on { yes } else { no };
        let jump = self.jump.to_string();
        let entries = [
            ("repeat", self.repeat.key()),
            ("screen_off", pick(self.play_screen_off, "play", "pause")),
            ("list", self.list_size.key()),
            ("clock", pick(self.clock, "on", "off")),
            ("up_next", pick(self.up_next, "on", "off")),
            ("color", pick(self.color, "on", "off")),
            ("theme", self.theme.key()),
            ("jump", jump.as_str()),
        ];
        for (key, value) in entries {
            if let Err(e) = ini::write(root, SETTINGS_FILE, key, value) {
                eprintln!("slot: music: {e}");
            }
        }
    }
}
