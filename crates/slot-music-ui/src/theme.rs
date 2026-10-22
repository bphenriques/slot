//! The music screens' colours, as a theme: dark, in slot's own menu colours, or light. Every
//! part is drawn in the theme's colours, so another theme is another instance of it.

use super::Ink;

/// A button hint's key cap, and the name on it, in either theme: as slot's own hints have them.
pub(super) const CAP_FACE: [u8; 3] = [0xf6, 0xf4, 0xef];
pub(super) const CAP_INK: [u8; 3] = [0x1a, 0x19, 0x17];

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct MusicTheme {
    /// Behind everything.
    pub ground: [u8; 3],
    /// The highlight, the empty picture box and the bars' tracks.
    pub bar: [u8; 3],
    /// What can be picked, what describes it, and what is behind or unavailable.
    pub lit: [u8; 3],
    pub dim: [u8; 3],
    pub faint: [u8; 3],
    /// How far Dynamic Color's bar goes from the album's colour to `lit`, to stand out.
    pub tint_to_lit: f32,
    /// How strongly Dynamic Color's wash shows over `ground`.
    pub wash: f32,
    /// How far a lifted row's highlight goes from `bar` to `lit`, as if raised.
    pub lift: f32,
    /// How strongly a heart shows off the highlight, as values are dim.
    pub heart_dim: f32,
    /// How strongly the visualizer's bars show beside the picture: faint, so they stay quiet.
    pub bars: f32,
    /// How strongly the rows behind a carried one show, faded back under it.
    pub behind: f32,
}

impl MusicTheme {
    /// Slot's menus: light ink on near black.
    pub fn dark() -> MusicTheme {
        MusicTheme {
            ground: [0x05, 0x05, 0x08],
            bar: [0x4d, 0x4d, 0x57],
            lit: [0xf6, 0xf4, 0xef],
            dim: [0x9a, 0x9a, 0xa4],
            faint: [0x5c, 0x5c, 0x66],
            tint_to_lit: 0.35,
            wash: 0.12,
            lift: 0.25,
            heart_dim: 0.55,
            bars: 0.14,
            behind: 0.4,
        }
    }

    /// Dark ink on paper, for slot's light appearance.
    pub fn light() -> MusicTheme {
        MusicTheme {
            ground: [0xf2, 0xf0, 0xec],
            bar: [0xc9, 0xc5, 0xbd],
            lit: [0x1a, 0x1a, 0x1d],
            dim: [0x66, 0x66, 0x6e],
            faint: [0xa6, 0xa6, 0xae],
            // Album colours are bright; on paper they need much more ink.
            tint_to_lit: 0.6,
            // Paper takes a colour less readily than black: a little more of it.
            wash: 0.16,
            lift: 0.25,
            heart_dim: 0.55,
            // Ink shows more on paper than light does on black.
            bars: 0.1,
            behind: 0.4,
        }
    }

    pub fn of(light: bool) -> MusicTheme {
        if light {
            MusicTheme::light()
        } else {
            MusicTheme::dark()
        }
    }

    pub fn ink(&self, ink: Ink) -> [u8; 3] {
        match ink {
            Ink::Lit => self.lit,
            Ink::Dim => self.dim,
            Ink::Faint => self.faint,
        }
    }
}

impl Default for MusicTheme {
    fn default() -> Self {
        MusicTheme::dark()
    }
}

/// A colour as the compositor takes it, with `alpha`.
pub fn rgba(c: [u8; 3], alpha: f32) -> [f32; 4] {
    let [r, g, b] = c.map(|c| c as f32 / 255.0);
    [r, g, b, alpha]
}
