//! The visualizer: Now Playing's background, moving with what is heard. The player is asked for
//! the levels only while they would be seen, and gives them back in its status.

use slot_player::{Bands, Command};

use super::{Effect, Music};
use crate::browse::View;

/// The steps a bar's height is shown in: finer than this moves no bar a pixel on a page.
pub(crate) const BAR_STEPS: u8 = 32;

/// How often the bars are drawn anew while they move: as a film is.
pub(crate) const FRAME_S: f64 = 1.0 / 30.0;

#[derive(Default)]
pub(crate) struct Visualizer {
    /// Whether the player was told to give the levels.
    told: bool,
    /// The levels the player last gave, while it was told to.
    bands: Option<Bands>,
}

impl Music {
    /// Whether the bars would be seen: on, on Now Playing, with the screen on.
    fn visualized(&self) -> bool {
        self.settings.visualizer && !self.dark && self.browser.view() == View::NowPlaying
    }

    /// Takes the levels the player gave, and tells it whether to go on giving them.
    pub(super) fn tell_visualizer(&mut self, bands: Option<Bands>) {
        let seen = self.visualized();
        self.visualizer.bands = bands.filter(|_| seen);
        if seen != self.visualizer.told {
            self.visualizer.told = seen;
            self.out.push(Effect::Player(Command::Spectrum(seen)));
        }
    }

    /// The bars Now Playing shows, each 0 to `BAR_STEPS - 1`: none while it is off or nothing is
    /// heard.
    pub(crate) fn bars(&self) -> Option<Bands> {
        let bands = self.visualizer.bands?;
        // Each level, 0 to 255, in as many steps.
        Some(bands.map(|level| level / (u8::MAX / BAR_STEPS + 1)))
    }
}
