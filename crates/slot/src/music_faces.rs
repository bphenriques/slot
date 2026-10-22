//! The music screen on slot's own: each frame, the view's faces of what the music shows, drawn
//! by the kit, its textures put into slot's compositor and its draws made slot's.

#![warn(clippy::match_bool, clippy::undocumented_unsafe_blocks)]

use slot_gfx::{Canvas, Compositor, Draw, TexId};
use slot_music_ui::{Canvas as KitCanvas, Draw as KitDraw, TexId as KitTexId};
use slot_music_view::{Frame, Textures};

use crate::app::App;

/// The music's view, and the compositor's texture for each of its own, by its number.
#[derive(Default)]
pub struct MusicText {
    view: slot_music_view::MusicText,
    textures: Vec<TexId>,
}

impl MusicText {
    /// Draws the music screen when it is shown, as the app has it now.
    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        let began = std::time::Instant::now();
        let Some(screen) = app.music_screen() else {
            self.view.hide();
            app.music_hidden();
            return;
        };
        let Canvas { w, h } = app.canvas();
        let frame = Frame {
            screen,
            art: app.music_art(),
            thumbs: app.music_thumbs(),
            theme: app.music_theme(),
            canvas: KitCanvas { w, h },
            now: app.now(),
        };
        let mut gpu = Gpu {
            compositor,
            textures: &mut self.textures,
        };
        let (faces, wake) = self.view.faces(frame, &mut gpu);
        let mut draws = Vec::new();
        slot_music_ui::draw_music(&faces, &mut draws);
        let draws = draws.into_iter().map(|d| slot_s(d, &self.textures));
        app.set_music_draws(draws.collect());
        app.music_drawn(wake);
        app.note_music_draw(began.elapsed());
    }
}

/// Slot's compositor, as the view puts textures: each of its own numbered in turn.
struct Gpu<'a> {
    compositor: &'a mut Compositor,
    textures: &'a mut Vec<TexId>,
}

impl Textures for Gpu<'_> {
    fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> KitTexId {
        self.textures
            .push(self.compositor.create_texture(w, h, rgba));
        KitTexId(self.textures.len() - 1)
    }

    fn update_texture(&mut self, id: KitTexId, w: u32, h: u32, rgba: &[u8]) {
        self.compositor
            .update_texture(self.textures[id.0], w, h, rgba);
    }
}

/// The kit's draw as slot's compositor draws it.
fn slot_s(draw: KitDraw, textures: &[TexId]) -> Draw {
    match draw {
        KitDraw::Rect { x, y, w, h, colour } => Draw::Rect { x, y, w, h, colour },
        KitDraw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha,
        } => Draw::Tex {
            x,
            y,
            w,
            h,
            tex: textures[tex.0],
            alpha,
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_music_menu_sits_where_slot_s_quick_menu_would() {
        use slot_ui::{quick_row_top, QuickRow};
        let first = QuickRow::MAIN[0];
        assert_eq!(
            slot_music_ui::music_menu_top(QuickRow::MAIN.len()),
            quick_row_top(first, &[]).round()
        );
    }
}
