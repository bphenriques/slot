//! The equalizer's presets: those slot ships (`music/music-eq.txt`), then the listener's own,
//! made on the device and kept in `Config/music-eq.txt`, a line each, to rename on a computer.

use slot_player::Band;

use super::settings::SETTINGS_FILE;
use super::Store;

pub const EQ_FILE: &str = "Config/music-eq.txt";
/// What slot ships: the bands every preset sets, how wide each is, and its own presets.
const SHIPPED: &str = include_str!("../../../../music/music-eq.txt");

/// How many bands every preset sets.
pub const BANDS: usize = 5;
/// How far each band goes up or down, in dB.
pub type Gains = [f32; BANDS];
/// The most a band goes either way in a preset made on the device.
pub const MOST_DB: f32 = 12.0;

impl Store {
    /// The equalizer's presets, and which `music.txt` names: 0, off, if none or one gone.
    pub fn equalizer(&self) -> (Presets, usize) {
        let presets = Presets::read(&self.read(EQ_FILE));
        let settings = super::file::parse(&self.read(SETTINGS_FILE));
        let on = settings.get("eq").map_or(0, |name| presets.find(name));
        (presets, on)
    }
}

/// A preset: its name, its gains, and whether it is the listener's own, to delete.
#[derive(Clone, PartialEq, Debug)]
pub struct Preset {
    pub name: String,
    pub gains: Gains,
    pub own: bool,
}

/// The presets, slot's then the listener's, and the bands they set.
#[derive(Clone, PartialEq, Debug)]
pub struct Presets {
    hz: [f32; BANDS],
    width: f32,
    presets: Vec<Preset>,
}

/// None at all, for a screen read without the music.
pub static NO_PRESETS: Presets = Presets {
    hz: [0.0; BANDS],
    width: 1.0,
    presets: Vec::new(),
};

impl Presets {
    /// Slot's, then the listener's own from `own`, the card's file: a line it cannot read is
    /// left out, as is one named as one of slot's.
    pub fn read(own: &str) -> Presets {
        let mut eq = Presets {
            presets: Vec::new(),
            ..NO_PRESETS.clone()
        };
        for (key, value) in lines(SHIPPED) {
            match key {
                "bands" => eq.hz = gains(value).unwrap_or_default(),
                "width" => eq.width = value.parse().unwrap_or(1.0),
                name => eq.add(name, gains(value), false),
            }
        }
        for (name, value) in lines(own).filter(|&(k, _)| k != "bands" && k != "width") {
            if eq.find(name) == 0 {
                eq.add(name, gains(value), true);
            }
        }
        eq
    }

    fn add(&mut self, name: &str, gains: Option<Gains>, own: bool) {
        if let Some(gains) = gains {
            let name = name.to_string();
            self.presets.push(Preset { name, gains, own });
        }
    }

    /// The listener's own presets as their file keeps them.
    pub fn encode(&self) -> String {
        let mut text =
            String::from("# Your equalizer presets, made in Music Settings. Rename one here.\n");
        for p in self.presets.iter().filter(|p| p.own) {
            let gains: Vec<String> = p.gains.iter().map(f32::to_string).collect();
            text.push_str(&format!("{} = {}\n", p.name, gains.join(", ")));
        }
        text
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.iter().map(|p| p.name.as_str())
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    /// Where the preset `name` is, from 1: 0 is off.
    pub fn find(&self, name: &str) -> usize {
        self.names().position(|n| n == name).map_or(0, |i| i + 1)
    }

    /// Preset `i`, from 1; none for off.
    pub fn get(&self, i: usize) -> Option<&Preset> {
        self.presets.get(i.checked_sub(1)?)
    }

    /// The name of preset `i`, from 1; none for off.
    pub fn name(&self, i: usize) -> Option<&str> {
        self.get(i).map(|p| p.name.as_str())
    }

    /// The gains of preset `i`, from 1; flat for off.
    pub fn gains(&self, i: usize) -> Gains {
        self.get(i).map_or([0.0; BANDS], |p| p.gains)
    }

    /// The centre of each band, in Hz.
    pub fn hz(&self) -> [f32; BANDS] {
        self.hz
    }

    /// The bands `gains` play through: none when flat, which is off.
    pub fn bands(&self, gains: Gains) -> Vec<Band> {
        if gains.iter().all(|&db| db == 0.0) {
            return Vec::new();
        }
        let band = |(&hz, &db)| Band {
            hz,
            db,
            q: self.width,
        };
        self.hz.iter().zip(&gains).map(band).collect()
    }

    /// Keeps `gains` as the listener's own preset `name`, after the rest: where it is, from 1.
    pub fn keep(&mut self, name: String, gains: Gains) -> usize {
        self.presets.push(Preset {
            name,
            gains,
            own: true,
        });
        self.presets.len()
    }

    /// Deletes preset `i`, from 1, if it is the listener's own; whether it did.
    pub fn delete(&mut self, i: usize) -> bool {
        let own = self.get(i).is_some_and(|p| p.own);
        if own {
            self.presets.remove(i - 1);
        }
        own
    }
}

/// `text`'s `key = value` lines, but for comments.
fn lines(text: &str) -> impl Iterator<Item = (&str, &str)> {
    let lines = text.lines().map(str::trim).filter(|l| !l.starts_with('#'));
    lines
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
}

/// A gain for each band, or none if one is missing or unreadable.
fn gains(value: &str) -> Option<Gains> {
    let numbers: Option<Vec<f32>> = value.split(',').map(|n| n.trim().parse().ok()).collect();
    numbers?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_s_presets_come_first_then_the_listener_s_own_from_the_card() {
        let shipped = Presets::read("");
        let n = shipped.len();
        assert!(
            n >= 3,
            "slot's own: {:?}",
            shipped.names().collect::<Vec<_>>()
        );
        assert!(shipped.get(1).is_some_and(|p| !p.own));
        let own =
            "bands = 1, 2\nMine = 3, -1, 0, 0, 2.5\nShort = 3\n# A note\nVocal = 1, 1, 1, 1, 1\n";
        let eq = Presets::read(own);
        assert_eq!(
            eq.len(),
            n + 1,
            "one short, and one named as slot's, left out"
        );
        assert_eq!(eq.find("Mine"), n + 1);
        assert_eq!(eq.gains(n + 1), [3.0, -1.0, 0.0, 0.0, 2.5]);
        assert!(eq.get(n + 1).is_some_and(|p| p.own));
        assert_eq!(eq.gains(0), [0.0; BANDS], "off is flat");
        assert_eq!((eq.find("Gone"), eq.name(0)), (0, None));
    }

    #[test]
    fn flat_plays_through_no_bands_and_the_rest_through_every_band() {
        let eq = Presets::read("");
        assert!(eq.bands([0.0; BANDS]).is_empty(), "flat is off");
        let bands = eq.bands([1.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(bands.len(), BANDS);
        assert_eq!((bands[0].hz, bands[0].db), (eq.hz()[0], 1.0));
    }

    #[test]
    fn the_listener_s_own_are_kept_deleted_and_written_back_but_slot_s_stay() {
        let mut eq = Presets::read("");
        let first = eq.len();
        let at = eq.keep("Custom 1".into(), [2.0, 0.0, -1.5, 0.0, 4.0]);
        assert_eq!(at, first + 1);
        let again = Presets::read(&eq.encode());
        assert_eq!(again, eq, "read back as written");
        assert!(!eq.delete(1), "slot's stay");
        assert!(eq.delete(at));
        assert_eq!(eq.len(), first);
        assert!(
            !eq.encode().contains('='),
            "none of slot's written: {}",
            eq.encode()
        );
    }
}
