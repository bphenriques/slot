//! What the music screens borrow from slot's own drawing, all in one place: when slot changes
//! one of these, it is mended here, and the rest of the music code follows its own modules.

pub(super) use crate::art::render_svg;
pub(super) use crate::battery::{cluster_width, draw_gauge, GAUGE_H};
pub(super) use crate::draw::{Canvas, Draw, TexId, OUT_H};
pub(super) use crate::footer::{draw_printed, Printed};
pub(super) use crate::hud::{FOOTER_INK_LIGHT, HUD_INK};
pub(super) use crate::icon::symbols_font;
pub(super) use crate::plate::{
    arrows_hint_coloured, arrows_hint_width, blit, cap_width, centred_hints, hint_coloured,
    hint_width, UndoFace, HINT_H, LEGEND_GAP,
};
pub(super) use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD, MENU_PX};
pub(super) use crate::quick_menu::{push, QUICK_EDGE, QUICK_PITCH};
pub(super) use crate::slot_chrome::{ease, edge, opening};
pub(super) use crate::text::{draw_centred, fit, label_font, line_width};
