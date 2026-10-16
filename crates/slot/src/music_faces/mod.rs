//! The music screen's textures. Lines are drawn once at full length and shown through a
//! window, so long ones scroll: the highlighted row in a list, every line on Now Playing.

mod list;
mod motion;
mod now_playing;

use slot_gfx::{Compositor, TexId};
use slot_input::Millis;
use slot_music::{text, Body, Legend, RowIcon, RowText};
use slot_ui::{
    marquee_offset, music_caret_face, music_heart_face, music_hint_face, music_letter_face,
    music_line_face, music_moon_face, music_ring_face, music_row_face, music_row_heart_face,
    music_status_face, music_tint_ink, music_title_w, music_window, BodyFaces, Ink, LineInk,
    MusicFace, MusicFaces, MusicTheme, UndoFace, MUSIC_STATUS,
};

use crate::app::App;
use motion::Motion;

/// How many steps a hold's ring fills in, each drawn once a colour.
const RING_STEPS: f32 = 40.0;

/// What a face is drawn from: a line in one style, or a list row at a size.
#[derive(Clone, PartialEq)]
enum Spec {
    Line(String, LineInk),
    Row { text: RowText, ink: Ink, px: u32 },
}

impl Spec {
    fn small(text: String) -> Spec {
        Spec::Line(text, LineInk::Small)
    }

    /// The face in `theme`, fitted to `width` when a little smaller is enough.
    fn face(&self, width: u32, theme: &MusicTheme) -> Option<UndoFace> {
        match self {
            Spec::Line(text, ink) => music_line_face(text, *ink, width, theme),
            Spec::Row { text, ink, px } => {
                let icon = text.icon.map(|RowIcon::Play| MUSIC_STATUS[0]);
                music_row_face(
                    icon,
                    &text.before,
                    &text.text,
                    &text.after,
                    *ink,
                    *px as f32,
                    width,
                    theme,
                )
            }
        }
    }
}

/// A line's face at full length, and the window of it on screen.
#[derive(Default)]
struct Line {
    /// What the face is of, and the width and theme it was drawn to.
    spec: Option<(Spec, u32, MusicTheme)>,
    full: Option<UndoFace>,
    tex: Option<TexId>,
    shown: Option<(u32, u32)>,
}

impl Line {
    /// The window `width` wide, scrolled along by `scroll` ms; drawn again only on a change.
    fn sync(
        &mut self,
        compositor: &mut Compositor,
        theme: MusicTheme,
        spec: Spec,
        width: u32,
        scroll: Option<Millis>,
    ) -> Option<MusicFace> {
        let key = (spec, width, theme);
        if self.spec.as_ref() != Some(&key) {
            self.full = key.0.face(width, &theme);
            self.spec = Some(key);
            self.shown = None;
        }
        let full = self.full.as_ref()?;
        let w = width.min(full.w);
        let x = scroll.map_or(0, |ms| marquee_offset(full.w - w, ms));
        if self.shown != Some((x, w)) {
            let face = music_window(full, x, w);
            put(compositor, &mut self.tex, face.w, face.h, &face.rgba);
            self.shown = Some((x, w));
        }
        Some((self.tex?, w, full.h))
    }
}

/// Uploads into `tex`, made on first use.
fn put(compositor: &mut Compositor, tex: &mut Option<TexId>, w: u32, h: u32, rgba: &[u8]) -> TexId {
    match *tex {
        Some(id) => {
            compositor.update_texture(id, w, h, rgba);
            id
        }
        None => *tex.insert(compositor.create_texture(w, h, rgba)),
    }
}

/// Faces drawn for a key and a theme, drawn again into the same textures when either changes.
struct Styled<K> {
    key: Option<(K, MusicTheme)>,
    faces: Vec<MusicFace>,
}

impl<K> Default for Styled<K> {
    fn default() -> Self {
        Styled {
            key: None,
            faces: Vec::new(),
        }
    }
}

impl<K: PartialEq> Styled<K> {
    fn sync(
        &mut self,
        compositor: &mut Compositor,
        key: K,
        theme: MusicTheme,
        draw: impl FnOnce(&K, &MusicTheme) -> Vec<UndoFace>,
    ) -> &[MusicFace] {
        let key = Some((key, theme));
        if self.key != key {
            let (k, theme) = key.as_ref().expect("just set");
            let faces = draw(k, theme);
            let mut was = self.faces.iter().map(|f| f.0);
            self.faces = faces
                .iter()
                .map(|f| {
                    (
                        put(compositor, &mut was.next(), f.w, f.h, &f.rgba),
                        f.w,
                        f.h,
                    )
                })
                .collect();
            self.key = key;
        }
        &self.faces
    }
}

/// Faces of a few symbols: carets for settings, the state symbols, the sleep timer's moon and
/// the favorite's heart.
#[derive(Copy, Clone)]
struct Glyphs {
    carets: [MusicFace; 2],
    status: [MusicFace; 5],
    heart: MusicFace,
    moon: MusicFace,
}

impl Glyphs {
    fn draw(theme: &MusicTheme) -> Vec<UndoFace> {
        let carets = [false, true].map(|right| music_caret_face(right, theme));
        let status = MUSIC_STATUS.map(|g| music_status_face(g, theme));
        let rest = [music_heart_face(theme), music_moon_face(theme)];
        carets.into_iter().chain(status).chain(rest).collect()
    }

    fn of(faces: &[MusicFace]) -> Glyphs {
        let at = |i: usize| faces[i];
        Glyphs {
            carets: [at(0), at(1)],
            status: [2, 3, 4, 5, 6].map(at),
            heart: at(7),
            moon: at(8),
        }
    }
}

/// Lines that keep their place while the screen is shown, each reused by its index.
#[derive(Default)]
struct Lines(Vec<Line>);

impl Lines {
    fn sync(
        &mut self,
        compositor: &mut Compositor,
        theme: MusicTheme,
        i: usize,
        spec: Spec,
        width: u32,
        scroll: Option<Millis>,
    ) -> Option<MusicFace> {
        if self.0.len() <= i {
            self.0.resize_with(i + 1, Line::default);
        }
        self.0[i].sync(compositor, theme, spec, width, scroll)
    }
}

/// When the line in focus started scrolling: a new highlight or track starts it over.
#[derive(Default)]
struct Scroll {
    focus: Option<(Option<usize>, String)>,
    since: Millis,
}

impl Scroll {
    fn ms(&mut self, focus: (Option<usize>, String), now: Millis) -> Millis {
        if self.focus.as_ref() != Some(&focus) {
            self.focus = Some(focus);
            self.since = now;
        }
        now.saturating_sub(self.since)
    }
}

#[derive(Default)]
pub struct MusicText {
    /// The colours every face is drawn in.
    theme: MusicTheme,
    header: Lines,
    lines: Lines,
    values: Lines,
    glyphs: Styled<()>,
    /// Each legend's hints.
    legends: Vec<(Legend, Styled<()>)>,
    /// The album the picture is of, and its face; and the picture it fades in over, whose
    /// texture takes the next one.
    art: Option<now_playing::Picture>,
    art_was: Option<now_playing::Picture>,
    /// The hearts list rows show, for the page size it was drawn at.
    row_heart: Styled<(bool, usize)>,
    /// The letter a list jumped to, and the ring a held key fills, by how far.
    letter: Styled<char>,
    ring: Styled<(u8, [u8; 3])>,
    scroll: Scroll,
    motion: Motion,
}

impl MusicText {
    /// A list row's heart at this page's size.
    fn row_heart(&mut self, compositor: &mut Compositor, menu: bool, per_page: usize) -> MusicFace {
        let draw =
            |&(menu, per_page): &_, theme: &_| vec![music_row_heart_face(menu, per_page, theme)];
        self.row_heart
            .sync(compositor, (menu, per_page), self.theme, draw)[0]
    }

    /// A legend's hints, each drawn once a theme.
    fn legend(&mut self, compositor: &mut Compositor, legend: Legend) -> Vec<(TexId, u32)> {
        let i = match self.legends.iter().position(|(l, _)| *l == legend) {
            Some(i) => i,
            None => {
                self.legends.push((legend, Styled::default()));
                self.legends.len() - 1
            }
        };
        let draw = |_: &_, theme: &MusicTheme| hint_faces(legend, theme).collect();
        let faces = self.legends[i].1.sync(compositor, (), self.theme, draw);
        faces.iter().map(|&(id, w, _)| (id, w)).collect()
    }

    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        let Some(screen) = app.music_screen() else {
            self.motion.hide();
            return;
        };
        let now = app.now();
        let (canvas, theme) = (app.canvas(), app.music_theme());
        self.theme = theme;
        self.motion.show(&screen, now);
        let glyphs = Glyphs::of(
            self.glyphs
                .sync(compositor, (), theme, |_, t| Glyphs::draw(t)),
        );
        // Menus are headless, as slot's are; only a notice or progress shows in the corner.
        let menu = match &screen.body {
            Body::List(list) => list.menu,
            Body::Message(_) => true,
            Body::NowPlaying(_) => false,
        };
        let clock = screen.clock && !menu;
        let title_w = music_title_w(clock, canvas);
        let title = match menu {
            true => None,
            false => self.header.sync(
                compositor,
                theme,
                0,
                Spec::small(screen.title),
                title_w,
                None,
            ),
        };
        let corner = self.header.sync(
            compositor,
            theme,
            1,
            Spec::small(screen.corner),
            title_w,
            None,
        );
        let legend = self.legend(compositor, screen.legend);
        // A key held to act fills a ring in the corner; Dynamic Color tints it on Now Playing,
        // as it does the bar.
        let ink = match (&screen.body, &self.art) {
            (Body::NowPlaying(np), Some(art)) if np.color => music_tint_ink(art.tint, &theme),
            _ => theme.lit,
        };
        let ring = screen.hold.and_then(|(_, done)| {
            let step = (done * RING_STEPS).round() as u8;
            let draw = |&(step, ink): &(u8, [u8; 3]), t: &MusicTheme| {
                vec![music_ring_face(step as f32 / RING_STEPS, ink, t)]
            };
            let faces = self.ring.sync(compositor, (step, ink), theme, draw);
            faces.first().copied()
        });
        let letter = screen.letter.and_then(|c| {
            let draw = |&c: &char, t: &MusicTheme| music_letter_face(c, t).into_iter().collect();
            let faces = self.letter.sync(compositor, c, theme, draw);
            faces.first().copied()
        });
        let body = match screen.body {
            Body::List(body) => {
                BodyFaces::List(list::faces(self, compositor, canvas, body, glyphs, now))
            }
            Body::Message(line) => {
                BodyFaces::List(list::message(self, compositor, canvas, line, now))
            }
            Body::NowPlaying(body) => BodyFaces::NowPlaying(now_playing::faces(
                self, app, compositor, canvas, body, glyphs, now,
            )),
        };
        app.set_music_faces(MusicFaces {
            title,
            corner,
            legend,
            body,
            motion: self.motion.at(now),
            clock,
            canvas,
            theme,
            letter,
            ring,
        });
    }
}

/// A legend's hints in `theme`, worded by `slot_music::text`. They hold still while a key is
/// held: the ring in the corner shows the hold.
fn hint_faces(legend: Legend, theme: &MusicTheme) -> impl Iterator<Item = UndoFace> + '_ {
    text::legend(legend).into_iter().map(move |hint| {
        let key = (hint.key != text::ARROWS).then_some(hint.key);
        music_hint_face(key, &hint.label(), theme)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_ui::{Canvas, LEGEND_GAP};

    #[test]
    fn every_music_legend_fits_across_the_screen() {
        let wide: Vec<String> = Legend::all()
            .into_iter()
            .filter_map(|legend| {
                let faces: Vec<UndoFace> = hint_faces(legend, &MusicTheme::dark()).collect();
                let gaps = LEGEND_GAP as u32 * (faces.len() as u32).saturating_sub(1);
                let w = faces.iter().map(|f| f.w).sum::<u32>() + gaps;
                (w as f32 > Canvas::FRAMED.w).then(|| format!("{legend:?} is {w} px"))
            })
            .collect();
        assert!(wide.is_empty(), "too wide: {wide:#?}");
    }
}
