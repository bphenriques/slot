//! Volume leveling from a file's ReplayGain tags: each song, or each album, brought to the same
//! loudness, never so far up that it clips.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Leveling {
    /// As recorded.
    #[default]
    Off,
    /// Every song alike: for shuffling.
    Track,
    /// Every album alike, its songs as loud as each other as mastered: the album's gain, or the
    /// song's where the album has none.
    Album,
}

/// The factors a file's tags give its song and its album; none where it has no tag.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Gains {
    track: Option<f32>,
    album: Option<f32>,
}

impl Gains {
    /// The gains in `path`'s tags; none when it has none or cannot be read.
    pub fn read(path: &Path) -> Gains {
        let options = ParseOptions::new()
            .read_cover_art(false)
            .read_properties(false);
        let tagged = Probe::open(path).and_then(|p| p.options(options).read());
        let Some(tag) = tagged
            .ok()
            .and_then(|t| t.primary_tag().or(t.first_tag()).cloned())
        else {
            return Gains::default();
        };
        let get = |key| tag.get_string(key).and_then(number);
        Gains {
            track: factor(
                get(ItemKey::ReplayGainTrackGain),
                get(ItemKey::ReplayGainTrackPeak),
            ),
            album: factor(
                get(ItemKey::ReplayGainAlbumGain),
                get(ItemKey::ReplayGainAlbumPeak),
            ),
        }
    }

    /// What every sample is multiplied by under `leveling`.
    pub fn factor(self, leveling: Leveling) -> f32 {
        match leveling {
            Leveling::Off => None,
            Leveling::Track => self.track,
            Leveling::Album => self.album.or(self.track),
        }
        .unwrap_or(1.0)
    }
}

/// `db` of gain as a factor, held where the `peak` it raises would clip.
fn factor(db: Option<f32>, peak: Option<f32>) -> Option<f32> {
    let gain = 10f32.powf(db? / 20.0);
    Some(match peak.filter(|&p| p > 0.0) {
        Some(peak) => gain.min(1.0 / peak),
        None => gain,
    })
}

/// "-8.50 dB" or "1.035", as a number.
fn number(text: &str) -> Option<f32> {
    text.trim()
        .trim_end_matches(['d', 'B', 'b'])
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gain_never_lifts_a_peak_past_full_scale() {
        assert_eq!(number("-8.50 dB"), Some(-8.5));
        let down = factor(Some(-6.0), Some(0.9)).unwrap();
        assert!((down - 0.501).abs() < 1e-3);
        // +6 dB would take a 0.8 peak to 1.6: held at 1.25, the peak at full scale.
        assert_eq!(factor(Some(6.0), Some(0.8)), Some(1.25));
        assert_eq!(factor(None, Some(0.8)), None);
        let gains = Gains {
            track: Some(0.5),
            album: None,
        };
        assert_eq!(gains.factor(Leveling::Off), 1.0);
        assert_eq!(
            gains.factor(Leveling::Album),
            0.5,
            "the song's, wanting the album's"
        );
    }
}
