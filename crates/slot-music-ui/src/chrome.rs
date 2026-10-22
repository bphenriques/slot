//! The kit's own drawing basics: its fonts, text set in capitals and fitted, the button hints,
//! pictures from SVG, and the sizes it shares with slot's menus, so the music screens sit beside
//! slot's own as one. Made as slot makes its own, and kept so by hand: the kit needs no frontend,
//! only a host to show its faces.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

pub(super) use super::draw::{Canvas, Draw, TexId};

use super::theme::{CAP_FACE, CAP_INK};

const LABEL_TTF: &[u8] = include_bytes!("../assets/label.ttf");
const SYMBOLS_TTF: &[u8] = include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf");

// Slot's menus' measures, which the music screens keep to.
/// The band at the top the header shares with slot's volume and status.
pub(super) const PLATE_H: f32 = 40.0;
/// Slot's quick menu: its rows' pitch, and the margin at each side of the screen.
pub(super) const QUICK_PITCH: f32 = 52.0;
pub const QUICK_EDGE: f32 = 32.0;
/// A menu row: its type, its height, and the room at each end of its words.
pub(super) const MENU_PX: f32 = 30.0;
pub(super) const MENU_H: u32 = 40;
pub const MENU_PAD: u32 = 18;
/// A button hint: its height, its key cap's, and the gap between hints.
pub(super) const HINT_H: u32 = 24;
pub(super) const CAP: u32 = 20;
pub const LEGEND_GAP: f32 = 36.0;
const CAP_PAD: u32 = 4;
const CAP_MAX_W: f32 = 64.0;
const CAP_GAP: u32 = 5;
const ARROW_GAP: u32 = 3;
const HINT_EDGE: u32 = 2;
const KEY_PX: f32 = 14.0;
const LABEL_PX: f32 = 16.0;
const LABEL_MIN_PX: f32 = 10.0;
const LABEL_MAX_W: f32 = 140.0;
const CARETS: [char; 2] = ['\u{f0d9}', '\u{f0da}'];

/// A face: its pixels, RGBA, and its size.
pub struct Bitmap {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// Slot's curve, smoothest at both ends.
pub(super) fn ease(u: f32) -> f32 {
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

/// A face drawn whole at `x`, `y`, on whole pixels.
pub(super) fn push(out: &mut Vec<Draw>, tex: TexId, x: f32, y: f32, w: u32, h: u32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

pub(super) fn label_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(LABEL_TTF, FontSettings::default()).ok())
        .as_ref()
}

pub(super) fn symbols_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(SYMBOLS_TTF, FontSettings::default()).ok())
        .as_ref()
}

/// Text set in capitals: its lines, its size and the room between its letters.
pub(super) struct Layout {
    pub lines: Vec<String>,
    pub px: f32,
    pub tracking: f32,
}

/// `text` in capitals at the largest size from `max_px` down to `min_px` that fits `max_lines`
/// lines `max_w` wide; at the smallest, words broken to fit.
pub(super) fn fit(
    font: &Font,
    text: &str,
    max_w: f32,
    max_lines: usize,
    max_px: f32,
    min_px: f32,
) -> Layout {
    let upper = text.to_uppercase();
    let fits = |lines: &[String], px, tracking| {
        lines.len() <= max_lines
            && lines
                .iter()
                .all(|l| line_width(font, l, px, tracking) <= max_w)
    };
    let mut px = max_px;
    while px > min_px {
        let tracking = tracking_for(px);
        let lines = wrap(font, &upper, px, tracking, max_w, false);
        if fits(&lines, px, tracking) {
            return Layout {
                lines,
                px,
                tracking,
            };
        }
        px -= 1.0;
    }
    let tracking = tracking_for(min_px);
    let lines = wrap(font, &upper, min_px, tracking, max_w, true);
    Layout {
        lines: lines.into_iter().take(max_lines).collect(),
        px: min_px,
        tracking,
    }
}

/// How wide `line` sets at `px`, `tracking` between its letters.
pub(super) fn line_width(font: &Font, line: &str, px: f32, tracking: f32) -> f32 {
    let n = line.chars().count() as f32;
    let w: f32 = line
        .chars()
        .map(|ch| font.metrics(ch, px).advance_width)
        .sum();
    w + tracking * (n - 1.0).max(0.0)
}

fn tracking_for(px: f32) -> f32 {
    (px * 0.10).round()
}

fn wrap(
    font: &Font,
    text: &str,
    px: f32,
    tracking: f32,
    max_w: f32,
    break_words: bool,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let parts = if break_words {
            break_word(font, word, px, tracking, max_w)
        } else {
            vec![word.to_string()]
        };
        for part in parts {
            let joined = if line.is_empty() {
                part.clone()
            } else {
                format!("{line} {part}")
            };
            if line.is_empty() || line_width(font, &joined, px, tracking) <= max_w {
                line = joined;
            } else {
                lines.push(std::mem::replace(&mut line, part));
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn break_word(font: &Font, word: &str, px: f32, tracking: f32, max_w: f32) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    for ch in word.chars() {
        part.push(ch);
        if line_width(font, &part, px, tracking) > max_w && part.chars().count() > 1 {
            part.pop();
            parts.push(std::mem::take(&mut part));
            part.push(ch);
        }
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

/// Draws `layout` centred in `dst`, `dst_w` by `dst_h` RGBA, in `colour`, over what is there.
pub(super) fn draw_centred(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    layout: &Layout,
    colour: [u8; 3],
) {
    let Some(font) = label_font() else { return };
    let Some(vm) = font.horizontal_line_metrics(layout.px) else {
        return;
    };
    let mut cover = vec![0u8; (dst_w * dst_h) as usize];
    let block_h = vm.new_line_size * layout.lines.len() as f32;
    let mut baseline = (dst_h as f32 - block_h) / 2.0 + vm.ascent;
    for line in &layout.lines {
        let mut pen = (dst_w as f32 - line_width(font, line, layout.px, layout.tracking)) / 2.0;
        for ch in line.chars() {
            let (m, glyph) = font.rasterize(ch, layout.px);
            let x = (pen + m.xmin as f32).round() as i32;
            let y = (baseline - (m.height as f32 + m.ymin as f32)).round() as i32;
            for (gy, row) in glyph.chunks(m.width.max(1)).enumerate() {
                for (gx, &a) in row.iter().enumerate() {
                    let (dx, dy) = (x + gx as i32, y + gy as i32);
                    if (0..dst_w as i32).contains(&dx) && (0..dst_h as i32).contains(&dy) {
                        let i = (dy as u32 * dst_w + dx as u32) as usize;
                        cover[i] = cover[i].max(a);
                    }
                }
            }
            pen += m.advance_width + layout.tracking;
        }
        baseline += vm.new_line_size;
    }
    for (px, a) in dst.chunks_exact_mut(4).zip(cover) {
        if a > 0 {
            blend(px, u32::from(a), colour);
        }
    }
}

fn blend(px: &mut [u8], a: u32, colour: [u8; 3]) {
    let under = u32::from(px[3]) * (255 - a) / 255;
    let out = a + under;
    for c in 0..3 {
        px[c] = ((u32::from(colour[c]) * a + u32::from(px[c]) * under + out / 2) / out) as u8;
    }
    px[3] = out as u8;
}

/// Copies `src`, `src_w` by `src_h`, into `dst`, `dst_w` wide, at `x`, `y`.
pub(super) fn blit(dst: &mut [u8], dst_w: u32, src: &[u8], src_w: u32, src_h: u32, x: u32, y: u32) {
    let row = (src_w * 4) as usize;
    for r in 0..src_h {
        let (from, to) = (
            (r * src_w * 4) as usize,
            (((y + r) * dst_w + x) * 4) as usize,
        );
        dst[to..to + row].copy_from_slice(&src[from..from + row]);
    }
}

/// How wide a key cap is for `key`'s name.
pub(super) fn cap_width(key: &str) -> u32 {
    let Some(font) = label_font() else {
        return CAP;
    };
    let layout = fit(font, key, CAP_MAX_W, 1, KEY_PX, KEY_PX);
    CAP.max(widest(font, &layout).ceil() as u32 + 2 * CAP_PAD)
}

/// How wide a hint is: its key's cap, then its words.
pub(super) fn hint_width(key: &str, label: &str) -> u32 {
    cap_width(key) + CAP_GAP + band_width(label) + HINT_EDGE
}

/// A hint: `key` named on a cap, and what it does beside it in `ink`.
pub(super) fn hint_coloured(key: &str, label: &str, ink: [u8; 3]) -> Bitmap {
    let w = hint_width(key, label);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];
    let cap_w = cap_width(key);
    let mut cap = solid(cap_w, CAP);
    if let Some(font) = label_font() {
        let layout = fit(font, key, cap_w as f32, 1, KEY_PX, KEY_PX);
        draw_centred(&mut cap, cap_w, CAP, &layout, CAP_INK);
    }
    blit(&mut rgba, w, &cap, cap_w, CAP, 0, (HINT_H - CAP) / 2);
    words(&mut rgba, w, label, ink, cap_w + CAP_GAP);
    Bitmap { rgba, w, h: HINT_H }
}

/// How wide the arrows' hint is: Left's cap and Right's, then its words.
pub(super) fn arrows_hint_width(label: &str) -> u32 {
    2 * CAP + ARROW_GAP + CAP_GAP + band_width(label) + HINT_EDGE
}

/// The arrows' hint: a cap for each, a caret on it, and what they do in `ink`.
pub(super) fn arrows_hint_coloured(label: &str, ink: [u8; 3]) -> Bitmap {
    let w = arrows_hint_width(label);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];
    for (i, caret) in CARETS.into_iter().enumerate() {
        let cap = glyph_cap(caret);
        blit(
            &mut rgba,
            w,
            &cap,
            CAP,
            CAP,
            i as u32 * (CAP + ARROW_GAP),
            (HINT_H - CAP) / 2,
        );
    }
    words(&mut rgba, w, label, ink, 2 * CAP + ARROW_GAP + CAP_GAP);
    Bitmap { rgba, w, h: HINT_H }
}

/// Hints in a row centred across `canvas`, `gap` apart: each one's face, width and left.
pub(super) fn centred_hints(
    canvas: Canvas,
    hints: &[(TexId, u32)],
    gap: f32,
) -> Vec<(TexId, u32, f32)> {
    let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
    let total = hints.iter().map(|&(_, w)| seen(w)).sum::<f32>()
        + gap * hints.len().saturating_sub(1) as f32;
    let mut x = canvas.centred(total).round();
    hints
        .iter()
        .map(|&(tex, w)| {
            let at = x.round();
            x += seen(w) + gap;
            (tex, w, at)
        })
        .collect()
}

/// A hint's words, `label` in `ink`, set into `rgba`, `w` wide, from `x`.
fn words(rgba: &mut [u8], w: u32, label: &str, ink: [u8; 3], x: u32) {
    let text_w = band_width(label);
    let mut band = vec![0u8; (text_w * HINT_H * 4) as usize];
    if let Some(font) = label_font() {
        let layout = fit(font, label, text_w as f32, 1, LABEL_PX, LABEL_MIN_PX);
        draw_centred(&mut band, text_w, HINT_H, &layout, ink);
    }
    blit(rgba, w, &band, text_w, HINT_H, x, 0);
}

fn band_width(label: &str) -> u32 {
    let Some(font) = label_font() else {
        return LABEL_MAX_W as u32;
    };
    let layout = fit(font, label, LABEL_MAX_W, 1, LABEL_PX, LABEL_MIN_PX);
    (widest(font, &layout).ceil() as u32).clamp(1, LABEL_MAX_W as u32)
}

fn widest(font: &Font, layout: &Layout) -> f32 {
    let widths = layout
        .lines
        .iter()
        .map(|l| line_width(font, l, layout.px, layout.tracking));
    widths.fold(0.0, f32::max)
}

/// A key cap `w` by `h`, blank.
fn solid(w: u32, h: u32) -> Vec<u8> {
    let [r, g, b] = CAP_FACE;
    [r, g, b, 255].repeat((w * h) as usize)
}

/// A key cap with `glyph`, from the symbols font, on it.
fn glyph_cap(glyph: char) -> Vec<u8> {
    let mut cap = solid(CAP, CAP);
    let Some(font) = symbols_font() else {
        return cap;
    };
    let (m, cover) = font.rasterize(glyph, KEY_PX);
    let (x0, y0) = (
        (CAP as i32 - m.width as i32) / 2,
        (CAP as i32 - m.height as i32) / 2,
    );
    for (gy, row) in cover.chunks(m.width.max(1)).enumerate() {
        for (gx, &a) in row.iter().enumerate() {
            let (dx, dy) = (x0 + gx as i32, y0 + gy as i32);
            if !(0..CAP as i32).contains(&dx) || !(0..CAP as i32).contains(&dy) {
                continue;
            }
            let (a, at) = (u32::from(a), ((dy as u32 * CAP + dx as u32) * 4) as usize);
            for k in 0..3 {
                let over = u32::from(CAP_INK[k]) * a + u32::from(cap[at + k]) * (255 - a);
                cap[at + k] = (over / 255) as u8;
            }
        }
    }
    cap
}

/// `svg` drawn `w` by `h`, RGBA, its colours as they are rather than over its coverage.
pub(super) fn render_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    let mut rgba = pixmap.data().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        let a = u32::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(rgba)
}
