//! The music screen without a device: slot's own faces, drawn by its own kit, put into pixels
//! here instead of the GPU, so a test can look at what a screen shows, and `task shots` can
//! keep a picture of each for a person (or an agent) to review.
//!
//! The music runs as slot runs it, on a card made for the test, with a player that plays into
//! silence at the speed a device would; the screen is drawn as the compositor would draw it,
//! a straight alpha blend of each texture and rectangle in turn.

use std::path::{Path, PathBuf};
use std::time::Duration;

use slot_gfx::{Canvas, Draw, TexId};
use slot_input::{Btn, RawEvent};
use slot_music::{Body, Handle, Music};
use slot_store::Appearance;
use slot_ui::music::{ART_BOX, THUMB_PX};

use super::super::{App, Phase};
use crate::music_faces::{MusicText, Textures};

/// Textures kept as their pixels, for drawing a screen on the CPU.
#[derive(Default)]
struct Pixels(Vec<(u32, u32, Vec<u8>)>);

impl Textures for Pixels {
    fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.0.push((w, h, rgba.to_vec()));
        TexId::from_raw(self.0.len() - 1)
    }

    fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        if let Some(i) = (0..self.0.len()).find(|&i| TexId::from_raw(i) == id) {
            self.0[i] = (w, h, rgba.to_vec());
        }
    }
}

/// A screen as drawn: its pixels, and what was drawn to make them.
struct Shot {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
    draws: Vec<Draw>,
}

impl Shot {
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [0, 1, 2, 3].map(|c| self.rgba[i + c])
    }

    /// Writes it as a PNG.
    fn save(&self, path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut png = png::Encoder::new(std::io::BufWriter::new(file), self.w, self.h);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&self.rgba)
            .unwrap();
    }
}

/// `draws` onto a canvas, as the compositor draws them: each texture or rectangle, tinted by its
/// colour, laid over what is there by its alpha. A texture is sampled at the nearest pixel.
fn raster(draws: &[Draw], textures: &Pixels, canvas: Canvas) -> Vec<u8> {
    let (w, h) = (canvas.w as u32, canvas.h as u32);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let white = (1, 1, vec![255u8; 4]);
    for draw in draws {
        let (x, y, dw, dh, tex, colour, turn, uv) = match *draw {
            Draw::Rect { x, y, w, h, colour } => (x, y, w, h, &white, colour, 0.0, WHOLE),
            Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha,
            } => (
                x,
                y,
                w,
                h,
                texture(textures, tex),
                [1.0, 1.0, 1.0, alpha],
                0.0,
                WHOLE,
            ),
            Draw::Part {
                x,
                y,
                w,
                h,
                tex,
                alpha,
                uv,
            } => (
                x,
                y,
                w,
                h,
                texture(textures, tex),
                [1.0, 1.0, 1.0, alpha],
                0.0,
                uv,
            ),
            Draw::Turned {
                x,
                y,
                w,
                h,
                tex,
                alpha,
                turn,
            } => (
                x,
                y,
                w,
                h,
                texture(textures, tex),
                [1.0, 1.0, 1.0, alpha],
                turn,
                WHOLE,
            ),
            Draw::Game | Draw::Shot { .. } => continue,
        };
        let (tw, th, pixels) = tex;
        let (cos, sin) = (turn.cos(), turn.sin());
        let mid = (x + dw / 2.0, y + dh / 2.0);
        // Turned about its middle, it may reach past its own box: every pixel it could touch.
        let reach = (dw.hypot(dh) / 2.0).ceil();
        let (x0, y0) = (
            (mid.0 - reach).max(0.0) as u32,
            (mid.1 - reach).max(0.0) as u32,
        );
        let (x1, y1) = (
            (mid.0 + reach).min(w as f32) as u32,
            (mid.1 + reach).min(h as f32) as u32,
        );
        for py in y0..y1 {
            for px in x0..x1 {
                // Back from the screen to the quad: undo the turn about the middle.
                let (lx, ly) = (px as f32 + 0.5 - mid.0, py as f32 + 0.5 - mid.1);
                let (qx, qy) = (
                    cos * lx + sin * ly + dw / 2.0,
                    -sin * lx + cos * ly + dh / 2.0,
                );
                if qx < 0.0 || qy < 0.0 || qx >= dw || qy >= dh {
                    continue;
                }
                let u = uv[0] + qx / dw * uv[2];
                let v = uv[1] + qy / dh * uv[3];
                let (sx, sy) = ((u * *tw as f32) as u32, (v * *th as f32) as u32);
                if sx >= *tw || sy >= *th {
                    continue;
                }
                let s = ((sy * tw + sx) * 4) as usize;
                let src: [f32; 4] =
                    [0, 1, 2, 3].map(|c| f32::from(pixels[s + c]) / 255.0 * colour[c]);
                let d = ((py * w + px) * 4) as usize;
                for c in 0..3 {
                    let was = f32::from(out[d + c]) / 255.0;
                    out[d + c] = ((src[c] * src[3] + was * (1.0 - src[3])) * 255.0).round() as u8;
                }
                out[d + 3] = 255;
            }
        }
    }
    out
}

/// A whole texture, as the compositor samples one that is not cropped.
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

fn texture(textures: &Pixels, id: TexId) -> &(u32, u32, Vec<u8>) {
    let i = (0..textures.0.len()).find(|&i| TexId::from_raw(i) == id);
    &textures.0[i.expect("a texture the screen was given")]
}

/// Where the sound goes: nowhere, at the pace a device takes it, so a song plays for as long
/// as it lasts.
struct Silence {
    rate: u32,
}

impl slot_player::Output for Silence {
    fn set_rate(&mut self, rate: u32) -> bool {
        self.rate = rate;
        true
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let frames = samples.len() as f64 / 2.0;
        std::thread::sleep(Duration::from_secs_f64(
            frames / f64::from(self.rate.max(1)),
        ));
        Ok(())
    }

    fn delay(&mut self) -> u64 {
        0
    }

    fn flush(&mut self) {}

    fn release(&mut self) {}
}

fn silent() -> Handle {
    Handle::on(Silence { rate: 48_000 })
}

/// A card to look at, as a listener's might be: albums by a few artists, invented, tagged as a
/// tagger would (title, artist, album, number, year, genre) with a cover drawn for each; a
/// compilation of several artists; a song with timed lyrics; and a book of three parts.
fn card(root: &Path) {
    // Artist, album, year, genre, two colours for its cover, and its songs.
    let albums: [(&str, &str, u32, &str, [[u8; 3]; 2], &[&str]); 7] = [
        (
            "Aurora Lane",
            "Northern Lights",
            2019,
            "Folk",
            [[30, 60, 120], [120, 200, 220]],
            &["Harbour Song", "Quiet Hours", "Snow on the Pines"],
        ),
        (
            "Aurora Lane",
            "First Light",
            2015,
            "Folk",
            [[200, 120, 60], [250, 220, 160]],
            &["Morning", "The Long Road North"],
        ),
        (
            "Glass Harbor",
            "Neon Tide",
            1987,
            "Electronic",
            [[40, 10, 70], [230, 60, 160]],
            &["Neon Tide", "Night Drive", "Afterglow"],
        ),
        (
            "Glass Harbor",
            "Static Bloom",
            1991,
            "Electronic",
            [[10, 40, 40], [60, 220, 170]],
            &["Static Bloom", "Signal"],
        ),
        (
            "The Midnight Trio",
            "Blue Hours",
            1962,
            "Jazz",
            [[15, 25, 60], [70, 110, 200]],
            &[
                "Opening Night",
                "A Long Walk Home Through the Rain at Midnight",
                "Last Call",
            ],
        ),
        (
            "Orchestra of the Coast",
            "Four Seasons Revisited",
            1978,
            "Classical",
            [[90, 60, 30], [220, 190, 120]],
            &[
                "Spring: Allegro",
                "Summer: Presto",
                "Autumn: Adagio",
                "Winter: Largo",
            ],
        ),
        (
            "Paper Kites Club",
            "Rooftop Summer",
            2023,
            "Indie Pop",
            [[240, 110, 90], [250, 230, 120]],
            &["Rooftop Summer", "Polaroid", "Holiday"],
        ),
    ];
    for (artist, album, year, genre, colours, songs) in albums {
        let dir = root.join("Music").join(artist).join(album);
        std::fs::create_dir_all(&dir).unwrap();
        for (n, title) in songs.iter().enumerate() {
            let path = dir.join(format!("{:02} {title}.wav", n + 1));
            wav(&path);
            tag(&path, title, artist, album, n + 1, year, genre);
        }
        cover(&dir.join("cover.png"), colours);
    }
    // A compilation: songs by several artists, no album artist named, kept as one album.
    let hits = root.join("Music/Compilations/Summer Hits");
    std::fs::create_dir_all(&hits).unwrap();
    for (n, (title, artist)) in [
        ("Sunburn", "Paper Kites Club"),
        ("Coastline", "Aurora Lane"),
        ("Heatwave", "Glass Harbor"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = hits.join(format!("{:02} {title}.wav", n + 1));
        wav(&path);
        tag(&path, title, artist, "Summer Hits", n + 1, 2021, "Pop");
    }
    cover(&hits.join("cover.png"), [[250, 160, 40], [240, 60, 90]]);
    std::fs::write(
        root.join("Music/Aurora Lane/Northern Lights/01 Harbour Song.lrc"),
        "[00:00.50]The boats come in at evening\n[00:02.00]The lamps along the quay\n[00:04.00]And every light is someone\n[00:06.00]Still waiting by the sea\n",
    )
    .unwrap();
    let book = root.join("Audiobooks/Kipling - Just So Stories");
    std::fs::create_dir_all(&book).unwrap();
    for part in [
        "01 How the Whale Got His Throat",
        "02 The Camel's Hump",
        "03 The Elephant's Child",
    ] {
        wav(&book.join(format!("{part}.wav")));
    }
    std::fs::create_dir_all(root.join("Config")).unwrap();
    std::fs::write(root.join("Config/music.txt"), "help = seen\n").unwrap();
}

/// Tags `path` as a tagger would.
fn tag(path: &Path, title: &str, artist: &str, album: &str, n: usize, year: u32, genre: &str) {
    use lofty::config::WriteOptions;
    use lofty::prelude::*;
    use lofty::tag::{Tag, TagType};
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.into());
    tag.set_artist(artist.into());
    tag.set_album(album.into());
    tag.set_track(n as u32);
    tag.set_genre(genre.into());
    tag.insert_text(ItemKey::RecordingDate, year.to_string());
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

/// A minute of a few soft tones swelling and fading in turn over a hush of noise, mono at 8 kHz:
/// something for the visualizer to show across its bands.
fn wav(path: &Path) {
    let (rate, secs) = (8_000u32, 60u32);
    let data = rate * secs * 2;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    // A small generator of noise, the same every run: a linear congruential one.
    let mut seed = 1u32;
    for n in 0..rate * secs {
        let t = n as f32 / rate as f32;
        let sample: f32 = [110.0, 220.0, 440.0, 880.0, 1760.0, 3000.0]
            .iter()
            .enumerate()
            .map(|(k, hz)| {
                let swell = 0.5 + 0.5 * (std::f32::consts::TAU * t * (0.4 + 0.3 * k as f32)).sin();
                0.1 * swell * (std::f32::consts::TAU * hz * t).sin()
            })
            .sum();
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sample = sample + 0.3 * (seed as f32 / u32::MAX as f32 - 0.5);
        b.extend_from_slice(&((sample * 32_767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// A square cover drawn here, free to use: a gradient from one colour to the other, and a circle
/// of the second low on it, as a sun on a horizon.
fn cover(path: &Path, [from, to]: [[u8; 3]; 2]) {
    let side = 300u32;
    let mut px = Vec::with_capacity((side * side * 3) as usize);
    let mix = |t: f32| {
        [0, 1, 2].map(|c| (f32::from(from[c]) + (f32::from(to[c]) - f32::from(from[c])) * t) as u8)
    };
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 - 150.0, y as f32 - 190.0);
            let sun = dx.hypot(dy) < 70.0;
            px.extend(if sun { to } else { mix(y as f32 / side as f32) });
        }
    }
    let file = std::fs::File::create(path).unwrap();
    let mut png = png::Encoder::new(std::io::BufWriter::new(file), side, side);
    png.set_color(png::ColorType::Rgb);
    png.write_header().unwrap().write_image_data(&px).unwrap();
}

/// slot with the music on screen, on a card of its own.
struct Screen {
    app: App,
    text: MusicText,
    textures: Pixels,
    /// When slot started, for its clock, which the frontend moves on each frame.
    began: std::time::Instant,
    /// How many frames a screenshot draws first, for what moves into place to arrive.
    settle: u32,
    _card: tempfile::TempDir,
}

impl Screen {
    fn new(fill: impl FnOnce(&Path)) -> Screen {
        let card = tempfile::tempdir().unwrap();
        fill(card.path());
        let mut app = App::new(Vec::new());
        app.set_panel((640, 480));
        app.music = Some(Music::spawn(card.path(), ART_BOX, THUMB_PX, silent));
        app.phase = Phase::Music;
        let mut screen = Screen {
            app,
            text: MusicText::default(),
            textures: Pixels::default(),
            began: std::time::Instant::now(),
            settle: 40,
            _card: card,
        };
        // Until the library is read and its pictures made.
        screen.wait(1500);
        screen
    }

    /// Lets `ms` go by, frame by frame, as slot does: its clock on, then the music polled.
    fn wait(&mut self, ms: u64) {
        for _ in 0..ms / 16 {
            self.app
                .tick_ms(self.began.elapsed().as_millis() as slot_input::Millis);
            self.app.poll_music();
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn tap(&mut self, btn: Btn) {
        self.app.music_button(RawEvent::Down(btn));
        self.wait(48);
        self.app.music_button(RawEvent::Up(btn));
        self.wait(96);
    }

    fn hold(&mut self, btn: Btn, ms: u64) {
        self.app.music_button(RawEvent::Down(btn));
        self.wait(ms);
        self.app.music_button(RawEvent::Up(btn));
        self.wait(96);
    }

    /// Up or Down to the row named `name`: on the page shown, or page by page until it is, on
    /// and then, past the last page, back.
    fn to(&mut self, name: &str) {
        let (mut turn, mut was) = (Btn::Right, (None, usize::MAX));
        for _ in 0..80 {
            let Some(slot_music::Screen {
                body: Body::List(list),
                ..
            }) = self.app.music_screen()
            else {
                panic!("no list for {name}");
            };
            match list.rows.iter().position(|r| r.text == name) {
                Some(at) if at == list.selected => return,
                Some(at) if at > list.selected => self.tap(Btn::Down),
                Some(_) => self.tap(Btn::Up),
                None => {
                    // Stuck where it was: the last page, so back the other way.
                    let page = (list.rows.first().map(|r| r.text.clone()), list.selected);
                    if page == was {
                        turn = Btn::Left;
                    }
                    was = page;
                    self.tap(turn);
                }
            }
        }
        panic!("never reached {name}");
    }

    fn open(&mut self, name: &str) {
        self.to(name);
        self.tap(Btn::A);
    }

    fn light(&mut self, light: bool) {
        self.app.state.appearance = if light {
            Appearance::Light
        } else {
            Appearance::Dark
        };
    }

    /// The screen as it is now, once what moves into place on a change has: drawn frame by
    /// frame a moment first, as slot draws it, so a screen that slides in has arrived.
    fn shot(&mut self) -> Shot {
        for _ in 0..self.settle {
            self.text.sync(&mut self.app, &mut self.textures);
            self.wait(16);
        }
        self.text.sync(&mut self.app, &mut self.textures);
        let mut draws = Vec::new();
        slot_ui::music::draw_music(&self.app.music_faces, &mut draws);
        let canvas = self.app.music_faces.canvas;
        let rgba = raster(&draws, &self.textures, canvas);
        Shot {
            w: canvas.w as u32,
            h: canvas.h as u32,
            rgba,
            draws,
        }
    }
}

/// The highlight of the row the cursor is on: the one bar across the screen below the header.
fn highlight(shot: &Shot) -> (f32, f32) {
    let bars = shot.draws.iter().filter_map(|d| match *d {
        Draw::Rect { x, y, w, h, .. } if x == 0.0 && w == shot.w as f32 && y > 0.0 && h < 100.0 => {
            Some((y, h))
        }
        _ => None,
    });
    bars.last().expect("a highlighted row")
}

#[test]
fn a_two_line_row_keeps_its_ink_inside_its_highlight_with_room_round_it() {
    let mut screen = Screen::new(card);
    screen.open("Library");
    screen.open("Albums");
    // Under it, "Paper Kites Club": a line reaching down, as a "p" does.
    screen.to("Rooftop Summer");
    let shot = screen.shot();
    let (top, h) = highlight(&shot);
    let ground = shot.pixel(shot.w - 2, shot.h - 2);
    let bar = shot.pixel(shot.w - 2, (top + h / 2.0) as u32);
    // The text's column, past the thumbnail; the lines just above and below the highlight are
    // ground, nothing of the row's text reaching out.
    let columns = 80..shot.w - 80;
    for y in [top - 2.0, top + h + 1.0].map(|y| y as u32) {
        let inked = columns
            .clone()
            .filter(|&x| shot.pixel(x, y) != ground)
            .count();
        assert_eq!(
            inked,
            0,
            "ink at y {y}, outside the highlight {top}..{}",
            top + h
        );
    }
    let inked: Vec<u32> = (top as u32..(top + h) as u32)
        .filter(|&y| columns.clone().any(|x| shot.pixel(x, y) != bar))
        .collect();
    let (first, last) = (
        inked.first().expect("its text drawn"),
        inked.last().unwrap(),
    );
    let (above, below) = (*first as f32 - top, top + h - 1.0 - *last as f32);
    assert!(
        above >= 5.0 && below >= 5.0,
        "room above {above}, below {below}"
    );
}

/// A tour of the music screens, each kept by `keep` under a name in the order a listener meets
/// them: the menus and lists, what plays, the START menus, and the moments between (a notice, a
/// letter, a hold filling, a song carried, the keys locked).
fn tour(s: &mut Screen, keep: &dyn Fn(&mut Screen, &str)) {
    let home = |s: &mut Screen| s.hold(Btn::B, 700);
    keep(s, "01-music");
    s.open("Library");
    keep(s, "02-library");
    s.open("Artists");
    keep(s, "03-artists");
    s.open("Glass Harbor");
    keep(s, "04-artist");
    s.tap(Btn::B);
    s.tap(Btn::B);
    s.open("Albums");
    s.to("Blue Hours");
    keep(s, "05-albums");
    // Moving fast by name shows the letter it is at.
    s.app.music_button(RawEvent::Down(Btn::Down));
    s.wait(900);
    s.app.music_button(RawEvent::Up(Btn::Down));
    keep_now(s, keep, "06-letter");
    s.to("Blue Hours");
    // A held, part way: the ring fills round its hint.
    s.app.music_button(RawEvent::Down(Btn::A));
    s.wait(300);
    keep_now(s, keep, "07-hold-ring");
    s.app.music_button(RawEvent::Up(Btn::B));
    s.app.music_button(RawEvent::Up(Btn::A));
    s.wait(200);
    keep(s, "08-album");
    s.tap(Btn::X);
    keep_now(s, keep, "09-notice-queued");
    s.to("A Long Walk Home Through the Rain at Midnight");
    s.tap(Btn::Start);
    keep(s, "10-song-menu");
    s.open("Song Info");
    keep(s, "11-song-info");
    home(s);
    s.open("Library");
    s.open("Albums");
    s.open("Blue Hours");
    s.open("Opening Night");
    s.wait(400);
    keep(s, "12-now-playing");
    s.tap(Btn::Start);
    keep(s, "13-playing-menu");
    s.to("Sleep Timer");
    s.tap(Btn::Right);
    keep(s, "14-sleep-timer");
    s.tap(Btn::B);
    s.tap(Btn::Select);
    keep(s, "15-queue");
    s.hold(Btn::Y, 700);
    keep(s, "16-queue-carry");
    s.tap(Btn::B);
    s.tap(Btn::Start);
    keep(s, "17-queue-menu");
    s.tap(Btn::B);
    s.tap(Btn::B);
    // Two shoulders held lock the keys, as in a pocket; the corner says so.
    s.app.music_button(RawEvent::Down(Btn::L1));
    s.app.music_button(RawEvent::Down(Btn::R1));
    s.wait(700);
    s.app.music_button(RawEvent::Up(Btn::R1));
    s.app.music_button(RawEvent::Up(Btn::L1));
    s.wait(100);
    keep_now(s, keep, "18-locked");
    s.app.music_button(RawEvent::Down(Btn::L1));
    s.app.music_button(RawEvent::Down(Btn::R1));
    s.wait(700);
    s.app.music_button(RawEvent::Up(Btn::R1));
    s.app.music_button(RawEvent::Up(Btn::L1));
    home(s);
    s.open("Library");
    s.open("Albums");
    s.open("Northern Lights");
    s.open("Harbour Song");
    s.wait(1200);
    s.tap(Btn::X);
    keep(s, "19-lyrics");
    home(s);
    s.open("Library");
    s.open("Songs");
    keep(s, "20-songs");
    s.tap(Btn::B);
    s.open("Genres");
    keep(s, "20a-genres");
    s.open("Jazz");
    keep(s, "20b-genre");
    s.tap(Btn::B);
    s.tap(Btn::B);
    s.open("Years");
    keep(s, "20c-years");
    s.open("1980–89");
    keep(s, "20d-decade");
    s.tap(Btn::B);
    s.tap(Btn::B);
    s.open("Albums");
    s.open("Summer Hits");
    keep(s, "20e-compilation");
    s.tap(Btn::B);
    s.tap(Btn::B);
    s.open("Folders");
    keep(s, "20f-folders");
    s.open("Paper Kites Club");
    keep(s, "20g-folder");
    home(s);
    s.open("Playlists");
    keep(s, "21-playlists");
    s.open("Recently Played");
    keep(s, "22-recently-played");
    home(s);
    s.open("Audiobooks");
    keep(s, "23-audiobooks");
    s.open("All Books");
    keep(s, "24-all-books");
    s.tap(Btn::Start);
    keep(s, "25-book-menu");
    s.tap(Btn::B);
    s.tap(Btn::A);
    s.wait(400);
    keep(s, "26-book");
    s.tap(Btn::X);
    keep(s, "26b-book-faster");
    s.tap(Btn::Select);
    keep(s, "27-chapters");
    s.tap(Btn::B);
    s.tap(Btn::Start);
    keep(s, "28-book-menu-playing");
    s.open("Bookmarks");
    keep(s, "29-bookmarks-none");
    home(s);
    s.open("Settings");
    keep(s, "30-settings");
    s.open("Music");
    keep(s, "31-settings-music");
    s.tap(Btn::B);
    s.open("Audiobooks");
    keep(s, "32-settings-audiobooks");
    s.tap(Btn::B);
    s.open("Help");
    keep(s, "33-help");
    s.tap(Btn::B);
    s.open("Advanced");
    keep(s, "34-advanced");
    s.open("Experimental");
    keep(s, "35-experimental");
    // The visualizer on, behind a song.
    home(s);
    s.open("Library");
    s.open("Albums");
    s.open("Blue Hours");
    s.open("Opening Night");
    home(s);
    s.open("Settings");
    s.open("Music");
    s.to("Visualizer");
    s.tap(Btn::Right);
    keep(s, "35b-settings-visualizer");
    s.tap(Btn::Select);
    s.wait(600);
    keep(s, "38-visualizer");
}

/// Keeps the screen as it is this moment, for what lasts only a moment (a notice, a letter,
/// a ring filling): a few frames drawn, not the settling a whole screen gets.
fn keep_now(s: &mut Screen, keep: &dyn Fn(&mut Screen, &str), name: &str) {
    s.settle = 4;
    keep(s, name);
    s.settle = 40;
}

/// Every music screen worth a look, as pictures in `target/shots`, light and dark: `task shots`.
#[test]
#[ignore = "pictures for a person to review: task shots"]
fn shots() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shots");
    std::fs::create_dir_all(&dir).unwrap();
    for light in [false, true] {
        let theme = if light { "light" } else { "dark" };
        let mut screen = Screen::new(card);
        screen.light(light);
        let keep = |s: &mut Screen, name: &str| {
            s.shot().save(&dir.join(format!("{theme}-{name}.png")));
        };
        tour(&mut screen, &keep);
    }
    let mut empty = Screen::new(|root| {
        std::fs::create_dir_all(root.join("Config")).unwrap();
        std::fs::write(root.join("Config/music.txt"), "help = seen\n").unwrap();
    });
    empty.shot().save(&dir.join("dark-36-empty-card.png"));
    empty.open("Library");
    empty.shot().save(&dir.join("dark-37-empty-library.png"));
    println!("pictures in {}", dir.display());
}
