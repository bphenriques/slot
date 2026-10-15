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
    /// `skip = dpad` (default): Left and Right change track on Now Playing, L1 and R1 seek.
    /// `skip = shoulders` swaps them.
    pub skip_with_dpad: bool,
    /// The size of library lists and the queue; menus keep slot's size.
    pub list_size: ListSize,
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

    pub fn label(self) -> &'static str {
        match self {
            ListSize::Small => "Small",
            ListSize::Normal => "Normal",
            ListSize::Large => "Large",
        }
    }

    fn parse(text: &str) -> Option<ListSize> {
        ListSize::ORDER
            .into_iter()
            .find(|r| r.label().eq_ignore_ascii_case(text))
    }

    /// Left (-1) and Right (1) step and stop at the ends; A (0) goes round.
    pub fn step(self, by: i32) -> ListSize {
        let n = ListSize::ORDER.len() as i32;
        let at = ListSize::ORDER.iter().position(|&r| r == self).unwrap_or(1) as i32;
        let to = match by {
            0 => (at + 1) % n,
            by => (at + by).clamp(0, n - 1),
        };
        ListSize::ORDER[to as usize]
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            repeat: Repeat::Off,
            play_screen_off: false,
            skip_with_dpad: true,
            list_size: ListSize::Normal,
        }
    }
}

impl Settings {
    pub fn read(root: &Path) -> Settings {
        let file = ini::read(root, SETTINGS_FILE);
        let is =
            |key: &str, value: &str| file.get(key).is_some_and(|v| v.eq_ignore_ascii_case(value));
        Settings {
            repeat: file
                .get("repeat")
                .and_then(|v| Repeat::parse(v))
                .unwrap_or_default(),
            play_screen_off: is("screen_off", "play"),
            skip_with_dpad: !is("skip", "shoulders"),
            list_size: file
                .get("list")
                .and_then(|v| ListSize::parse(v))
                .unwrap_or(ListSize::Normal),
        }
    }

    pub fn write(&self, root: &Path) {
        let pick = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_string();
        let entries = [
            ("repeat", self.repeat.label().to_lowercase()),
            ("screen_off", pick(self.play_screen_off, "play", "pause")),
            ("skip", pick(self.skip_with_dpad, "dpad", "shoulders")),
            ("list", self.list_size.label().to_lowercase()),
        ];
        for (key, value) in entries {
            if let Err(e) = ini::write(root, SETTINGS_FILE, key, &value) {
                eprintln!("slot: music: {e}");
            }
        }
    }
}
