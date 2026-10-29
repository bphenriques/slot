//! The Authentic shader: each console's unlit, reflective LCD as it looks in room light. The
//! reflector behind the liquid crystal is the brightest the picture gets; a pixel only takes
//! light away from it.
//!
//! Sources:
//! - Game Boy Light shades: SameBoy's GBL palette; nobody has measured the Light.
//! - Game Boy and Pocket shade colours: BGB's "reality" palettes, photographed under
//!   calibrated light and corrected with a colour checker (bgb.bircd.org/reality); its
//!   lightest and darkest shades. The compositing (each dot a share of ink over the lit
//!   panel, the dot grid, the shadow the dots cast on the reflector, the lit panel around the picture) follows
//!   Harlequin's gameboy shader and its dmg-3x preset from the libretro collection.
//! - GBC and GBA colour response: Pokefan531's 2026 spectrophotometer measurements
//!   (github.com/Pokefan531/Handheld-Colorspace-Shaders), per-code ramps and matrices.
//! - Panel white tint: the Handheld Color Space Project's reflectance readings. Its contrast,
//!   near 4:1 on the GBC and 5-6:1 on the GBA, is a 45/0 instrument reading; photographs of
//!   the screens show more, so black here is an eighth of white on the GBC and a fourteenth
//!   on the GBA.

/// The screen an Authentic picture is drawn on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Screen {
    /// The original Game Boy's yellow-green panel.
    Dmg,
    /// The Game Boy Pocket's grey-olive panel.
    Pocket,
    /// The Game Boy Light's panel, lit from behind in blue-green.
    Light,
    Gbc,
    Gba,
}

pub(crate) struct Look {
    /// A Game Boy's lit panel, behind and around the dots, in sRGB; a colour screen's lit
    /// white, linear.
    pub light: [f32; 3],
    /// A Game Boy's dot colour, in sRGB; a colour screen's black, linear.
    pub dark: [f32; 3],
    /// A colour screen's response, from the measured ramp's linear light to linear sRGB,
    /// column by column for the input's red, green and blue.
    pub response: [f32; 9],
    /// Game Boy (1) or colour (0), which ramp (0 GBA, 1 GBC), and how much the gaps between
    /// columns and rows darken.
    pub lcd: [f32; 4],
    /// The share of the last frame a pixel keeps each frame while it turns lighter, and
    /// while it turns darker: the LCD's slow response.
    pub ghost: [f32; 2],
}

/// The share of the last frame kept each frame by a pixel that settles with time constant
/// `ms`, at the Game Boy's 59.73 frames a second.
fn keep(ms: f32) -> f32 {
    (-1000.0 / 59.73 / ms).exp()
}

/// Each 5-bit code's light on the GBA and the GBC, in gamma 2.2: Pokefan531's measured ramps.
pub(crate) const RAMPS: [[u8; 32]; 2] = [
    [
        0, 3, 6, 10, 15, 25, 36, 50, 62, 71, 82, 92, 102, 109, 116, 124, 131, 138, 145, 152, 160,
        168, 176, 184, 193, 202, 211, 218, 229, 240, 249, 255,
    ],
    [
        0, 8, 15, 26, 37, 49, 67, 85, 97, 110, 121, 134, 140, 148, 154, 160, 170, 180, 188, 196,
        202, 207, 212, 217, 222, 228, 232, 237, 246, 252, 254, 255,
    ],
];

/// The ramps as a 32 by 1 RGBA texture: the GBA's in red, the GBC's in green.
pub(crate) fn ramp_texture() -> Vec<u8> {
    (0..32)
        .flat_map(|i| [RAMPS[0][i], RAMPS[1][i], 0, 255])
        .collect()
}

fn rgb(hex: u32) -> [f32; 3] {
    [16, 8, 0].map(|s| ((hex >> s) & 0xff) as f32 / 255.0)
}

fn linear(srgb: [f32; 3]) -> [f32; 3] {
    srgb.map(|c| c.powf(2.2))
}

/// How opaque a Game Boy's white dot and its black one are: an off dot still shades the
/// panel a little, and an on one lets a little through (dmg-3x's baseline and contrast).
const DOT_OFF: f32 = 0.08;
const DOT_ON: f32 = 0.92;

/// How bright a colour screen's white is drawn, in sRGB: dimmer than the device's white, as
/// an unlit screen is, but not so dim that the game is hard to see.
const LIT: f32 = 0.82;

impl Screen {
    pub(crate) fn look(self) -> Look {
        // A dot's colour over the lit panel at its opacity: solve for the panel and the dot
        // colour that put a white dot on the lightest measured shade and a black one on the
        // darkest. The panel around the picture is then the one its white sits on.
        let mono = |lightest, darkest, ghost| {
            let (light, dark) = (rgb(lightest), rgb(darkest));
            let step = [0, 1, 2].map(|i| (dark[i] - light[i]) / (DOT_ON - DOT_OFF));
            let panel = [0, 1, 2].map(|i| (light[i] - DOT_OFF * step[i]).clamp(0.0, 1.0));
            let ink = [0, 1, 2].map(|i| (panel[i] + step[i]).clamp(0.0, 1.0));
            Look {
                light: panel,
                dark: ink,
                response: [0.0; 9],
                lcd: [1.0, 0.0, 0.0, 0.0],
                ghost,
            }
        };
        // White is the panel's tint at LIT; black is a share of white's light, a little
        // tinted the way each panel's black measured.
        let colour =
            |tint, black: f32, black_tint: [f32; 3], lum: f32, m: [f32; 9], gaps, ghost| {
                let white = linear(rgb(tint).map(|c| c * LIT));
                let dark = [0, 1, 2].map(|i| white[i] * black * black_tint[i]);
                Look {
                    light: white,
                    dark,
                    response: m.map(|v| v * lum),
                    lcd: gaps,
                    ghost,
                }
            };
        match self {
            // The DMG's STN panel clears slowly and darkens faster (brickboy's 61 and 21 ms);
            // the Pocket's and the Light's answer about twice as fast. Nobody has measured them.
            Screen::Dmg => mono(0x759833, 0x2e615a, [keep(61.0), keep(21.0)]),
            Screen::Pocket => mono(0xadbf92, 0x5a635c, [keep(30.0), keep(10.0)]),
            // SameBoy's Game Boy Light shades.
            Screen::Light => mono(0x00cbdf, 0x00778c, [keep(30.0), keep(10.0)]),
            Screen::Gbc => colour(
                0xedfffc,
                0.12,
                [1.0, 1.0, 1.0],
                0.8989,
                [
                    0.8944, 0.1091, 0.1447, 0.2179, 0.6312, 0.2837, -0.1125, 0.2597, 0.5716,
                ],
                [0.0, 1.0, 0.10, 0.25],
                [keep(16.0), keep(8.0)],
            ),
            Screen::Gba => colour(
                0xdef6ff,
                0.07,
                [1.02, 0.97, 1.06],
                0.8915,
                [
                    0.8988, 0.1163, 0.1831, 0.2229, 0.6217, 0.2438, -0.1217, 0.2620, 0.5731,
                ],
                [0.0, 0.0, 0.45, 0.20],
                [keep(20.0), keep(10.0)],
            ),
        }
    }
}

/// What the LCD shows after its slow response: two textures at the game's own size, the
/// last one read while the next is drawn. Tiny next to the screen, and none of it on the CPU.
pub(crate) struct Ghost {
    prog: gl::types::GLuint,
    u_keep: gl::types::GLint,
    tex: [gl::types::GLuint; 2],
    fbo: [gl::types::GLuint; 2],
    shown: usize,
    primed: bool,
}

impl Ghost {
    pub fn new() -> Result<Self, crate::surface::GfxError> {
        use crate::pipeline::{SRC_H, SRC_W};
        let prog = crate::shaders::program(crate::shaders::BLIT_VERT, crate::shaders::GHOST_FRAG)?;
        let tex = [0, 1].map(|_| {
            crate::gl::texture(SRC_W, SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None)
        });
        let mut fbo = [0; 2];
        unsafe {
            gl::GenFramebuffers(2, fbo.as_mut_ptr());
            for (f, t) in fbo.iter().zip(tex) {
                gl::BindFramebuffer(gl::FRAMEBUFFER, *f);
                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    t,
                    0,
                );
            }
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_prev"), 1);
            gl::Uniform4f(
                crate::gl::uniform_location(prog, "u_uv"),
                0.0,
                0.0,
                1.0,
                1.0,
            );
        }
        let u_keep = crate::gl::uniform_location(prog, "u_keep");
        Ok(Ghost {
            prog,
            u_keep,
            tex,
            fbo,
            shown: 0,
            primed: false,
        })
    }

    /// Start again from the next frame, with nothing of an older picture in it.
    pub fn reset(&mut self) {
        self.primed = false;
    }

    /// The screen after a new frame from `game`, each pixel `keep`ing part of what it showed.
    pub fn respond(&mut self, game: gl::types::GLuint, keep: [f32; 2], quad: &crate::quad::Quad) {
        use crate::pipeline::{SRC_H, SRC_W};
        let next = 1 - self.shown;
        let keep = if self.primed { keep } else { [0.0; 2] };
        unsafe {
            let mut fb = 0;
            let mut view = [0; 4];
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fb);
            gl::GetIntegerv(gl::VIEWPORT, view.as_mut_ptr());
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo[next]);
            gl::Viewport(0, 0, SRC_W as i32, SRC_H as i32);
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_keep, keep[0], keep[1]);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.tex[self.shown]);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, game);
            quad.draw();
            gl::BindFramebuffer(gl::FRAMEBUFFER, fb as gl::types::GLuint);
            gl::Viewport(view[0], view[1], view[2], view[3]);
        }
        self.shown = next;
        self.primed = true;
    }

    /// The screen as it shows now, once a frame has gone through it.
    pub fn texture(&self) -> Option<gl::types::GLuint> {
        self.primed.then_some(self.tex[self.shown])
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(2, self.fbo.as_ptr());
            gl::DeleteTextures(2, self.tex.as_ptr());
            gl::DeleteProgram(self.prog);
        }
    }
}
