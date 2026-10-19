//! The settings on the card, `Config/music.txt`: `key = value` lines, which a person may edit
//! and comment, so a change writes its one key and leaves the rest as it was.

use slot_store::ini;

use super::Store;
use crate::playback::player::Repeat;
use crate::settings::{ListSize, Settings, Theme, BOOSTS, FADES, JUMPS};

pub const SETTINGS_FILE: &str = "Config/music.txt";

impl Store {
    pub fn settings(&self) -> Settings {
        Settings::decode(&self.read(SETTINGS_FILE))
    }

    /// Writes what differs from `was`.
    #[cfg(test)]
    pub fn save_settings(&self, now: &Settings, was: &Settings) {
        for (key, value) in changes(now, was) {
            self.set(SETTINGS_FILE, key, value);
        }
    }
}

/// The keys `now` sets otherwise than `was`, as it was read or last written: one key a change,
/// rather than the whole file over again for each.
pub fn changes(now: &Settings, was: &Settings) -> Vec<(&'static str, String)> {
    let before = was.entries();
    let entries = now.entries().into_iter();
    entries.filter(|e| !before.contains(e)).collect()
}

impl Settings {
    /// What `text` sets; anything missing or unknown is its default.
    pub fn decode(text: &str) -> Settings {
        let file = ini::parse(text);
        let get = |key: &str| file.get(key).map(String::as_str);
        let is = |key: &str, value: &str| get(key).is_some_and(|v| v.eq_ignore_ascii_case(value));
        // One of `choices`, or the first.
        let number = |key: &str, choices: &[u32]| {
            let n = get(key).and_then(|v| v.parse().ok());
            n.filter(|n| choices.contains(n)).unwrap_or(choices[0])
        };
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
            up_next: !is("up_next", "off"),
            // Named in music.txt, and found among the presets: `Store::equalizer`.
            eq: 0,
            color: !is("color", "off"),
            theme: get("theme").and_then(theme).unwrap_or(Theme::Slot),
            jump: get("jump")
                .and_then(|v| v.parse().ok())
                .filter(|j| JUMPS.contains(j))
                .unwrap_or(30),
            mono: is("mono", "on"),
            // Once track or album, chosen now by how the queue plays.
            leveling: get("leveling").is_some_and(|v| !v.eq_ignore_ascii_case("off")),
            crossfade: number("crossfade", &FADES),
            skip_silence: is("skip_silence", "on"),
            boost: number("boost", &BOOSTS),
            // Once a choice of pair, now either, unless off.
            pocket: !is("pocket", "off"),
            pocket_buzz: !is("pocket_buzz", "off"),
            perf: is("perf", "on"),
            log: is("log", "on"),
            scrobbler: is("scrobbler", "on"),
        }
    }

    /// Every setting as `music.txt` writes it.
    fn entries(&self) -> Vec<(&'static str, String)> {
        let pick = |on: bool, yes: &'static str, no: &'static str| if on { yes } else { no };
        let jump = self.jump.to_string();
        let (crossfade, boost) = (self.crossfade.to_string(), self.boost.to_string());
        let entries = [
            ("repeat", self.repeat.key()),
            ("screen_off", pick(self.play_screen_off, "play", "pause")),
            ("list", self.list_size.key()),
            ("up_next", pick(self.up_next, "on", "off")),
            ("color", pick(self.color, "on", "off")),
            ("theme", self.theme.key()),
            ("jump", jump.as_str()),
            ("mono", pick(self.mono, "on", "off")),
            ("leveling", pick(self.leveling, "on", "off")),
            ("crossfade", crossfade.as_str()),
            ("skip_silence", pick(self.skip_silence, "on", "off")),
            ("boost", boost.as_str()),
            ("pocket", pick(self.pocket, "on", "off")),
            ("pocket_buzz", pick(self.pocket_buzz, "on", "off")),
            ("perf", pick(self.perf, "on", "off")),
            ("log", pick(self.log, "on", "off")),
            ("scrobbler", pick(self.scrobbler, "on", "off")),
        ];
        entries.map(|(key, value)| (key, value.to_string())).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Setting;
    use slot_player::Leveling;

    #[test]
    fn a_change_writes_its_one_key_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(SETTINGS_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "theme = dark\n").unwrap();
        let store = Store::open(dir.path());
        let was = store.settings();
        let mut now = was;
        now.change(Setting::Jump, -1);
        store.save_settings(&now, &was);
        store.flush();
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            text.lines().count(),
            2,
            "the one changed, beside what was there: {text}"
        );
        assert_eq!(store.settings(), now);
    }

    #[test]
    fn leveling_once_by_song_or_album_is_now_on() {
        for (was, on) in [
            ("off", false),
            ("track", true),
            ("album", true),
            ("on", true),
        ] {
            assert_eq!(
                Settings::decode(&format!("leveling = {was}")).leveling,
                on,
                "{was}"
            );
        }
        let on = Settings {
            leveling: true,
            ..Settings::default()
        };
        assert_eq!(on.level(false), Leveling::Album, "in order");
        assert_eq!(on.level(true), Leveling::Track, "shuffled");
    }
}
