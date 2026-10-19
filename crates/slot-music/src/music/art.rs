//! Album pictures: the playing album's, read by the worker as the album changes, and the labels
//! a scan found missing, made one at a time while no game runs.

use super::worker::{Done, Job};
use super::{Effect, Music};
use crate::browse::View;
use crate::library::covers::{self, Art, Covers};
use crate::library::AlbumId;

/// The album pictures: the labels still to make, the album whose picture was asked for, the
/// picture shown, and whether Now Playing showed it last frame; and the next song's album,
/// whose picture is read ahead, so a skip to it shows it at once.
pub(crate) struct Pictures {
    pub(crate) covers: Covers,
    pub(crate) shown: Option<AlbumId>,
    pub(crate) art: Option<Art>,
    seen: bool,
    next: Option<AlbumId>,
    ahead: Option<Art>,
}

impl Default for Pictures {
    fn default() -> Self {
        Pictures {
            covers: Covers::new(Vec::new()),
            shown: None,
            art: None,
            seen: false,
            next: None,
            ahead: None,
        }
    }
}

impl Music {
    /// Asks for the next missing label, and for the playing album's picture when the album
    /// changes. On Now Playing the last picture stays up while the next loads, to fade between
    /// them; anywhere else it is let go, so Now Playing opens on the new one, not the last.
    pub(crate) fn show_art(&mut self, game: bool) {
        if !game {
            if let Some(make) = self.pictures.covers.next(&self.root, &self.library) {
                self.work(Job::Make(make));
            }
        }
        let album = self.player.current().map(|t| self.library.tracks[t].album);
        if album != self.pictures.shown {
            self.pictures.shown = album;
            if !self.pictures.seen {
                self.pictures.art = None;
            }
            let ahead = self
                .pictures
                .ahead
                .take()
                .filter(|a| Some(a.album) == album);
            match (album, ahead) {
                (Some(_), Some(art)) => self.pictures.art = Some(art),
                (Some(album), None) => self.load_art(album),
                (None, _) => self.pictures.art = None,
            }
        }
        let next = self.player.up_next();
        let next = next
            .map(|t| self.library.tracks[t].album)
            .filter(|&n| Some(n) != album);
        if next != self.pictures.next {
            (self.pictures.next, self.pictures.ahead) = (next, None);
            if let Some(next) = next {
                self.load_art(next);
            }
        }
        self.pictures.seen = self.browser.view() == View::NowPlaying && !self.dark;
    }

    /// What the worker did, for this library: a label made, or a picture read.
    pub(crate) fn on_art(&mut self, done: Done) {
        match done {
            Done::Made(album) => {
                self.pictures.covers.made();
                if self.pictures.shown == Some(album)
                    && self.pictures.art.as_ref().map(|a| a.album) != Some(album)
                {
                    self.load_art(album);
                }
            }
            Done::Loaded(album, art) if self.pictures.shown == Some(album) => {
                self.pictures.art = art
            }
            Done::Loaded(album, art) if self.pictures.next == Some(album) => {
                self.pictures.ahead = art
            }
            _ => {}
        }
    }

    fn load_art(&mut self, album: AlbumId) {
        let label = covers::label(&self.root, &self.library, album);
        self.work(Job::Load { album, label });
    }

    /// Gives the worker `job`, under the library's generation as it is now.
    pub(crate) fn work(&mut self, job: Job) {
        self.out.push(Effect::Work(self.generation, job));
    }
}
