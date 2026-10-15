//! The music screen's textures. Lines are drawn once at full length and shown through a
//! window, so long ones scroll: the highlighted row in a list, every line on Now Playing.

use slot_gfx::{Compositor, TexId};
use slot_input::Millis;
use slot_music::{Legend, Repeat, RowIcon, RowText, Status};
use slot_ui::{
    marquee_offset, music_action_px, music_legend_faces, music_line_face, music_row_face,
    music_row_px, music_scrolls, music_status_face, music_text_width, music_window,
    quick_caret_face, LineInk, MusicFace, MusicFaces, MusicLegends, UndoFace, MUSIC_STATUS,
    MUSIC_TITLE_W,
};

use crate::app::App;

/// What a face is drawn from: a line in one style, or a list row at a size.
#[derive(Clone, PartialEq)]
enum Spec {
    Line(String, LineInk),
    Row { text: RowText, dim: bool, px: u32 },
}

impl Spec {
    fn face(&self) -> Option<UndoFace> {
        match self {
            Spec::Line(text, ink) => music_line_face(text, *ink),
            Spec::Row { text, dim, px } => {
                let icon = text.icon.map(|RowIcon::Play| MUSIC_STATUS[0]);
                music_row_face(
                    icon,
                    &text.before,
                    &text.text,
                    &text.after,
                    *dim,
                    *px as f32,
                )
            }
        }
    }
}

#[derive(Default)]
struct Line {
    spec: Option<Spec>,
    full: Option<UndoFace>,
    tex: Option<TexId>,
    shown: Option<(u32, u32)>,
}

impl Line {
    fn sync(
        &mut self,
        compositor: &mut Compositor,
        spec: Spec,
        width: u32,
        scroll: Option<Millis>,
    ) -> Option<MusicFace> {
        if self.spec.as_ref() != Some(&spec) {
            self.full = spec.face();
            self.spec = Some(spec);
            self.shown = None;
        }
        let full = self.full.as_ref()?;
        let w = width.min(full.w);
        let x = scroll.map_or(0, |ms| marquee_offset(full.w - w, ms));
        if self.shown != Some((x, w)) {
            let face = music_window(full, x, w);
            match self.tex {
                Some(id) => compositor.update_texture(id, face.w, face.h, &face.rgba),
                None => self.tex = Some(compositor.create_texture(face.w, face.h, &face.rgba)),
            }
            self.shown = Some((x, w));
        }
        Some((self.tex?, w, full.h))
    }
}

#[derive(Default)]
pub struct MusicText {
    lines: Vec<Line>,
    values: Vec<Line>,
    carets: Option<[MusicFace; 2]>,
    status: Option<[MusicFace; 5]>,
    legends: Option<MusicLegends<(TexId, u32)>>,
    art: Option<(usize, MusicFace)>,
    scrolling: Option<(Option<usize>, RowText)>,
    since: Millis,
}

impl MusicText {
    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        let Some(screen) = app.music_screen() else {
            return;
        };
        let now = app.now();
        let mut upload = |f: UndoFace| (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h);
        let legends = self.legends.get_or_insert_with(|| {
            music_legend_faces().map(|f| {
                let w = f.w;
                (upload(f).0, w)
            })
        });
        let legend = match screen.legend {
            Legend::Main => &legends.main,
            Legend::List => &legends.list,
            Legend::Queue => &legends.queue,
            Legend::NowPlaying => &legends.now_playing,
            Legend::Settings => &legends.settings,
            Legend::Back => &legends.back,
        }
        .clone();
        let carets = *self
            .carets
            .get_or_insert_with(|| [false, true].map(|right| upload(quick_caret_face(right))));
        let glyphs = *self
            .status
            .get_or_insert_with(|| MUSIC_STATUS.map(|g| upload(music_status_face(g))));
        let status = screen.status.map_or(Vec::new(), |s| shown(s, glyphs));
        let art = self.art(app, compositor);
        // A new highlight, or a new track on Now Playing, scrolls from the start.
        let focus = screen.selected.unwrap_or(0);
        let key = (
            screen.selected,
            screen.rows.get(focus).cloned().unwrap_or_default(),
        );
        if self.scrolling.as_ref() != Some(&key) {
            self.scrolling = Some(key);
            self.since = now;
        }
        let scroll = now.saturating_sub(self.since);
        let now_playing = screen.status.is_some();
        let width = music_text_width(screen.art_box);
        let per_page = screen.per_page;
        let (row_px, action_px) = (music_row_px(per_page), music_action_px(per_page));
        // Now Playing shows its queue place under the bar, not by the title.
        let (page, place) = match now_playing {
            true => (String::new(), screen.page),
            false => (screen.page, String::new()),
        };
        let small = |text: String| Spec::Line(text, LineInk::Small);
        let wanted = [
            (small(screen.title), MUSIC_TITLE_W, None),
            (small(page), MUSIC_TITLE_W, None),
            (small(place), MUSIC_TITLE_W, None),
            (small(screen.time), MUSIC_TITLE_W, None),
        ]
        .into_iter()
        .chain(screen.rows.into_iter().enumerate().map(|(i, text)| {
            // Now Playing: the track lit, the rest smaller and dim. Lists: action rows smaller
            // and dim, played tracks dim.
            let spec = match (now_playing, i) {
                (true, 0) => Spec::Line(text.text, LineInk::Lit),
                (true, _) => Spec::Line(text.text, LineInk::Second),
                _ if text.action => Spec::Row {
                    text,
                    dim: true,
                    px: action_px as u32,
                },
                _ => Spec::Row {
                    dim: text.dim,
                    text,
                    px: row_px as u32,
                },
            };
            let moving = music_scrolls(screen.selected, i);
            (spec, width, moving.then_some(scroll))
        }));
        let mut faces = Vec::new();
        for (i, (spec, w, scroll)) in wanted.enumerate() {
            if self.lines.len() <= i {
                self.lines.push(Line::default());
            }
            faces.push(self.lines[i].sync(compositor, spec, w, scroll));
        }
        let values = screen
            .values
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                if self.values.len() <= i {
                    self.values.push(Line::default());
                }
                // Dim, lit on the highlighted row, as in the quick menu.
                let spec = Spec::Row {
                    text: RowText::plain(text.unwrap_or_default()),
                    dim: screen.selected != Some(i),
                    px: row_px as u32,
                };
                self.values[i].sync(compositor, spec, MUSIC_TITLE_W, None)
            })
            .collect();
        let rows = faces.split_off(4);
        app.set_music_faces(MusicFaces {
            title: faces[0],
            page: faces[1],
            place: faces[2],
            time: faces[3],
            rows,
            values,
            // Arrows only where Left and Right change a value: settings.
            carets: (screen.legend == Legend::Settings).then_some(carets),
            selected: screen.selected,
            art_box: screen.art_box,
            progress: screen.progress,
            art,
            legend,
            status,
            per_page,
            actions: screen.actions,
        });
    }

    /// The playing album's picture, uploaded when the album changes.
    fn art(&mut self, app: &App, compositor: &mut Compositor) -> Option<MusicFace> {
        let a = app.music_art()?;
        if let Some((_, face)) = self.art.filter(|(album, _)| *album == a.album) {
            return Some(face);
        }
        let tex = match self.art {
            Some((_, (id, _, _))) => {
                compositor.update_texture(id, a.w, a.h, &a.rgba);
                id
            }
            None => compositor.create_texture(a.w, a.h, &a.rgba),
        };
        self.art = Some((a.album, (tex, a.w, a.h)));
        Some((tex, a.w, a.h))
    }
}

/// Now Playing's state symbols: play or pause, then shuffle and repeat when on.
fn shown(s: Status, [play, pause, repeat, repeat_one, shuffle]: [MusicFace; 5]) -> Vec<MusicFace> {
    let mut out = vec![if s.playing { play } else { pause }];
    if s.shuffled {
        out.push(shuffle);
    }
    match s.repeat {
        Repeat::All => out.push(repeat),
        Repeat::One => out.push(repeat_one),
        Repeat::Off => {}
    }
    out
}
