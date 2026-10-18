//! Recently Played on the card: a plain playlist, newest first, with paths from the card's root,
//! so other players can read it too.

use super::{Card, Kept};
use crate::playback::recent::Recent;

impl Kept for Recent {
    const FILE: &'static str = "Config/music-recent.m3u8";

    fn encode(&self, card: Card) -> String {
        let mut out = String::from("#EXTM3U\n");
        for &t in &self.tracks {
            out.push_str(&card.path(t));
            out.push('\n');
        }
        out
    }

    /// Songs no longer in the library are left out.
    fn decode(text: &str, card: Card) -> Recent {
        let index = card.lib.index();
        let tracks = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| index.get(card.root.join(l).as_path()).copied())
            .collect();
        Recent::new(tracks)
    }
}
