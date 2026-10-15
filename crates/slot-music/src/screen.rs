//! What the music screens show, built from the player, the browser and the library.

use std::time::Duration;

use crate::browse::{RowText, View};
use crate::muse::MuseState;
use crate::player::Repeat;
use crate::{Art, Music};

/// How long a notice like "+N" shows in the corner.
const NOTICE: Duration = Duration::from_millis(1500);

/// Which button hints a screen shows.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Legend {
    Main,
    List,
    Queue,
    NowPlaying,
    Settings,
    Back,
}

/// What the music screen shows. Lists highlight a row; Now Playing does not.
#[derive(Clone, PartialEq, Debug)]
pub struct Screen {
    pub title: String,
    pub rows: Vec<RowText>,
    /// A setting's value beside its row; empty when the screen has none.
    pub values: Vec<Option<String>>,
    pub selected: Option<usize>,
    pub page: String,
    pub legend: Legend,
    /// Room is kept for the album picture whether or not it has loaded, so nothing moves.
    pub art_box: bool,
    /// How far along the screen's work is, 0 to 1: the track playing, or the album pictures.
    pub progress: Option<f32>,
    /// Now Playing's state, drawn as symbols by the title.
    pub status: Option<Status>,
    /// Now Playing's time into the track and its length, small under the bar.
    pub time: String,
    /// Rows a page holds (which sets their size), and how many at the top are actions.
    pub per_page: usize,
    pub actions: usize,
}

/// Whether music plays, how it repeats, and whether the queue is shuffled.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Status {
    pub playing: bool,
    pub repeat: Repeat,
    pub shuffled: bool,
}

impl Music {
    pub fn screen(&self) -> Screen {
        // Only library lists wait for the library to be read.
        let view = self.browser.view();
        let lists = !matches!(
            view,
            View::Main | View::Settings | View::NowPlaying | View::Queue
        );
        if lists && self.loading.is_some() {
            return self.message("Music", "Reading the library");
        }
        if lists && self.library.tracks.is_empty() {
            return self.message("Music", "No music in Music/Artist/Album");
        }
        if self.browser.view() == View::NowPlaying {
            return self.now_playing();
        }
        if self.browser.view() == View::Queue && self.player.queue().tracks.is_empty() {
            return self.message("Queue", "Queue is empty");
        }
        let page = self
            .browser
            .page(&self.library, self.player.queue(), &self.settings);
        // The main screen also reports the album pictures being made.
        let covers = self
            .covers
            .progress()
            .filter(|_| self.browser.view() == View::Main);
        let notice = self
            .notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE)
            .map(|(text, _)| text.clone());
        let corner = match (notice, covers) {
            (Some(notice), _) => notice,
            (None, Some((done, of))) => format!("Optimizing {done}/{of}"),
            // Lists count their items; the short main and settings screens do not.
            (None, None) if matches!(self.browser.view(), View::Main | View::Settings) => {
                String::new()
            }
            (None, None) => match page.item {
                Some(i) => format!("{i}/{}", page.items),
                None => page.items.to_string(),
            },
        };
        Screen {
            title: page.title,
            rows: page.rows,
            values: page.values,
            selected: Some(page.selected),
            page: corner,
            legend: match self.browser.view() {
                View::Main => Legend::Main,
                View::Queue => Legend::Queue,
                View::Settings => Legend::Settings,
                _ => Legend::List,
            },
            art_box: false,
            progress: covers.map(|(done, of)| done as f32 / of as f32),
            status: None,
            time: String::new(),
            per_page: page.per_page,
            actions: page.actions,
        }
    }

    /// The playing album's picture, on Now Playing only.
    pub fn art(&self) -> Option<&Art> {
        let shown = self.browser.view() == View::NowPlaying && self.player.current().is_some();
        self.art.as_ref().filter(|_| shown)
    }

    fn now_playing(&self) -> Screen {
        if !self.connected {
            return self.message("Now Playing", "Player not running");
        }
        let Some(track) = self.player.current() else {
            return self.message("Now Playing", "Nothing playing");
        };
        let lib = &self.library;
        let (at, len) = self.player.time();
        let (n, of) = self.player.place();
        Screen {
            title: "Now Playing".into(),
            rows: vec![
                RowText::plain(&lib.tracks[track].title),
                RowText::plain(&lib.artist_of(track).name),
                RowText::plain(&lib.album_of(track).name),
            ],
            values: Vec::new(),
            selected: None,
            page: format!("{n}/{of}"),
            legend: Legend::NowPlaying,
            art_box: true,
            progress: (len > 0.0).then(|| (at / len).clamp(0.0, 1.0) as f32),
            status: Some(Status {
                playing: self.player.state() == MuseState::Playing,
                repeat: self.player.repeat(),
                shuffled: self.player.shuffled(),
            }),
            time: format!("{} / {}", clock(at), clock(len)),
            per_page: crate::browse::per_page(self.browser.view(), &self.settings),
            actions: 0,
        }
    }

    fn message(&self, title: &str, text: &str) -> Screen {
        Screen {
            title: title.into(),
            rows: vec![RowText::plain(text)],
            values: Vec::new(),
            selected: None,
            page: String::new(),
            legend: Legend::Back,
            art_box: false,
            progress: None,
            status: None,
            time: String::new(),
            per_page: crate::browse::per_page(self.browser.view(), &self.settings),
            actions: 0,
        }
    }
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}
