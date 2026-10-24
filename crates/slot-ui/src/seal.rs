//! The holographic seal a cart wears once its ROM can earn RetroAchievements offline. It is
//! baked into the cart's face, so it stays on the cart through the core picker, the slot and
//! the eject, like a real sticker would.
use crate::bitmaps;
use crate::cart::CartFace;

pub const SEAL_W: u32 = bitmaps::SEAL_W;
pub const SEAL_H: u32 = bitmaps::SEAL_H;

/// How far the seal's centre sits inside the label's bottom-left corner. Most of it is on the
/// plastic, so it reads as stuck on after the label was printed.
const INSET: u32 = 6;

/// Where the seal goes on a cart whose label box is `label`, in that cart's pixels.
pub fn seal_origin(label: (u32, u32, u32, u32)) -> (u32, u32) {
    let (lx, ly, _, lh) = label;
    (
        (lx + INSET).saturating_sub(SEAL_W / 2),
        (ly + lh - INSET).saturating_sub(SEAL_H / 2),
    )
}

pub(crate) fn paste_seal(face: &mut CartFace, label: (u32, u32, u32, u32)) {
    let (ox, oy) = seal_origin(label);
    let seal = bitmaps::seal();
    for y in 0..SEAL_H.min(face.h.saturating_sub(oy)) {
        for x in 0..SEAL_W.min(face.w.saturating_sub(ox)) {
            let src = ((y * SEAL_W + x) * 4) as usize;
            let a = seal[src + 3] as u32;
            if a == 0 {
                continue;
            }
            let d = (((y + oy) * face.w + x + ox) * 4) as usize;
            // Source over, in straight alpha: clear plastic under the seal is not opaque.
            let under = face.rgba[d + 3] as u32 * (255 - a);
            let out = a * 255 + under;
            for c in 0..3 {
                face.rgba[d + c] =
                    ((seal[src + c] as u32 * a * 255 + face.rgba[d + c] as u32 * under + out / 2)
                        / out) as u8;
            }
            face.rgba[d + 3] = ((out + 127) / 255) as u8;
        }
    }
}

pub const GLINT_FRAMES: usize = 8;

/// A band of light crossing the seal, frame by frame, cut to the seal's outline. The shelf
/// plays it once over a cart that has just been sealed, as foil catches the light.
pub fn glint_faces() -> Vec<CartFace> {
    let seal = bitmaps::seal();
    let (w, h) = (SEAL_W as f32, SEAL_H as f32);
    (0..GLINT_FRAMES)
        .map(|n| {
            // The band's centre runs from beyond the top-left to beyond the bottom-right.
            let t = n as f32 / (GLINT_FRAMES - 1) as f32;
            let centre = -0.35 + 1.7 * t;
            let mut rgba = vec![0u8; (SEAL_W * SEAL_H * 4) as usize];
            for y in 0..SEAL_H {
                for x in 0..SEAL_W {
                    let i = ((y * SEAL_W + x) * 4) as usize;
                    let along = (x as f32 / w + y as f32 / h) / 2.0;
                    let light = (1.0 - ((along - centre) / 0.16).abs()).max(0.0);
                    let a = (light * light * 230.0 * seal[i + 3] as f32 / 255.0) as u8;
                    rgba[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
            CartFace {
                rgba,
                w: SEAL_W,
                h: SEAL_H,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_nothing_the_seal_keeps_its_own_colours() {
        let mut face = CartFace {
            rgba: vec![0; (SEAL_W * SEAL_H * 4) as usize],
            w: SEAL_W,
            h: SEAL_H,
        };
        paste_seal(&mut face, (SEAL_W / 2 - INSET, 0, 0, SEAL_H / 2 + INSET));
        let seal = bitmaps::seal();
        for (got, want) in face.rgba.chunks_exact(4).zip(seal.chunks_exact(4)) {
            if want[3] == 0 {
                assert_eq!(got, [0, 0, 0, 0]);
            } else {
                assert_eq!(got, want, "a seal pixel was mixed with the empty face");
            }
        }
    }

    #[test]
    fn over_clear_plastic_the_seal_mixes_by_both_alphas() {
        let mut face = CartFace {
            rgba: [200, 100, 50, 204].repeat((SEAL_W * SEAL_H) as usize),
            w: SEAL_W,
            h: SEAL_H,
        };
        paste_seal(&mut face, (SEAL_W / 2 - INSET, 0, 0, SEAL_H / 2 + INSET));
        let seal = bitmaps::seal();
        let (i, px) = seal
            .chunks_exact(4)
            .enumerate()
            .find(|(_, p)| p[3] > 40 && p[3] < 200)
            .expect("the seal has soft edges");
        let (a, da) = (px[3] as f32 / 255.0, 204.0 / 255.0);
        let out = a + da * (1.0 - a);
        let got = &face.rgba[i * 4..i * 4 + 4];
        for (c, under) in [200.0, 100.0, 50.0].into_iter().enumerate() {
            let want = (px[c] as f32 * a + under * da * (1.0 - a)) / out;
            assert!((got[c] as f32 - want).abs() <= 1.0, "{got:?} vs {want}");
        }
        assert!((got[3] as f32 - out * 255.0).abs() <= 1.0);
    }
}
