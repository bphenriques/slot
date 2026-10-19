//! The equalizer's presets, from `Config/music-eq.txt`: the bands, how wide each is, and each
//! preset's gains. Data, not code: the card's file if there is one, else the one slot ships,
//! the same as `music-eq.txt.example` beside it.

use slot_player::Band;

use super::settings::SETTINGS_FILE;
use super::Store;

pub const EQ_FILE: &str = "Config/music-eq.txt";
/// What slot ships, read when the card has no file of its own.
const SHIPPED: &str = include_str!("../../../../music/music-eq.txt");

impl Store {
    /// The equalizer's presets, and which `music.txt` names: 0, off, if none or one gone.
    pub fn equalizer(&self) -> (Presets, usize) {
        let presets = Presets::read(&self.read(EQ_FILE));
        let settings = slot_store::ini::parse(&self.read(SETTINGS_FILE));
        let on = settings.get("eq").map_or(0, |name| presets.find(name));
        (presets, on)
    }
}

/// The presets, in order, each by its name with its gain at each band.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Presets {
    bands: Vec<f32>,
    width: f32,
    presets: Vec<(String, Vec<f32>)>,
}

impl Presets {
    /// The card's file's, `text`; or, it empty or naming none, what slot ships.
    pub fn read(text: &str) -> Presets {
        let own = Presets::decode(text);
        if own.presets.is_empty() {
            Presets::decode(SHIPPED)
        } else {
            own
        }
    }

    /// `text` as presets: `bands = …`, `width = …`, then `Name = gain, …`; a line it cannot read
    /// is left out, as is a preset with a gain short.
    fn decode(text: &str) -> Presets {
        let numbers =
            |v: &str| -> Option<Vec<f32>> { v.split(',').map(|n| n.trim().parse().ok()).collect() };
        let mut eq = Presets {
            width: 1.0,
            ..Presets::default()
        };
        let lines = text.lines().map(str::trim).filter(|l| !l.starts_with('#'));
        for (key, value) in lines.filter_map(|l| l.split_once('=')) {
            let (key, value) = (key.trim(), value.trim());
            match key {
                "bands" => eq.bands = numbers(value).unwrap_or_default(),
                "width" => eq.width = value.parse().unwrap_or(1.0),
                name => {
                    if let Some(gains) = numbers(value).filter(|g| g.len() == eq.bands.len()) {
                        eq.presets.push((name.to_string(), gains));
                    }
                }
            }
        }
        eq
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.presets.iter().map(|(name, _)| name.as_str())
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    /// Where the preset `name` is, from 1: 0 is off.
    pub fn find(&self, name: &str) -> usize {
        self.names().position(|n| n == name).map_or(0, |i| i + 1)
    }

    /// The name of preset `i`, from 1; none for off.
    pub fn name(&self, i: usize) -> Option<&str> {
        self.presets.get(i.checked_sub(1)?).map(|(n, _)| n.as_str())
    }

    /// The bands preset `i` plays through, from 1; none for off.
    pub fn bands(&self, i: usize) -> Vec<Band> {
        let Some((_, gains)) = i.checked_sub(1).and_then(|i| self.presets.get(i)) else {
            return Vec::new();
        };
        let band = |(&hz, &db)| Band {
            hz,
            db,
            q: self.width,
        };
        self.bands.iter().zip(gains).map(band).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_read_from_the_file_and_without_one_from_what_slot_ships() {
        let text = "bands = 100, 1000\nwidth = 0.7\nWarm = 3, -1\nShort = 3\n# A note\n";
        let eq = Presets::read(text);
        assert_eq!(
            eq.names().collect::<Vec<_>>(),
            ["Warm"],
            "a preset a gain short left out"
        );
        assert_eq!((eq.find("Warm"), eq.find("Gone")), (1, 0));
        assert_eq!(eq.name(1), Some("Warm"));
        assert_eq!(eq.name(0), None, "off");
        let bands = eq.bands(1);
        assert_eq!(
            bands[1],
            Band {
                hz: 1000.0,
                db: -1.0,
                q: 0.7
            }
        );
        assert!(eq.bands(0).is_empty());
        let shipped = Presets::read("");
        assert!(
            shipped.len() >= 3,
            "slot's own: {:?}",
            shipped.names().collect::<Vec<_>>()
        );
        assert!(shipped.bands(1).len() == 5);
    }
}
