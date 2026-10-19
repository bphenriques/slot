//! Playlists: Favorites first, always listed, which slot adds songs to and takes them from;
//! the queue kept as a new one; and the rest, read from the card to play. Their files are the
//! store's.

use std::path::PathBuf;

use crate::library::{Library, TrackId};

pub struct Playlist {
    pub name: String,
    pub path: PathBuf,
    pub tracks: Vec<TrackId>,
}

/// Favorites, the first playlist: songs only, kept as any other playlist file.
impl Library {
    pub fn is_favorite(&self, t: TrackId) -> bool {
        self.playlists
            .first()
            .is_some_and(|f| f.tracks.contains(&t))
    }

    /// Puts song `t` in Favorites, or takes it out: Favorites' place, 0, to write it by. A queue
    /// dealt from Favorites before keeps its songs.
    pub fn toggle_favorite(&mut self, t: TrackId) -> Option<usize> {
        let favorites = self.playlists.first_mut()?;
        if favorites.tracks.contains(&t) {
            favorites.tracks.retain(|&f| f != t)
        } else {
            favorites.tracks.push(t)
        }
        Some(0)
    }

    /// Adds `tracks` as a new playlist beside Favorites: `name`, or with a number after it when
    /// that is taken. Its place among the playlists, to write it by.
    pub fn add_playlist(&mut self, name: &str, tracks: Vec<TrackId>) -> Option<usize> {
        let dir = self.playlists.first()?.path.parent()?.to_path_buf();
        let taken = |n: &str| {
            self.playlists
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(n))
        };
        let name = (1..)
            .map(|i| match i {
                1 => name.to_string(),
                i => format!("{name} {i}"),
            })
            .find(|n| !taken(n))?;
        self.playlists.push(Playlist {
            path: dir.join(format!("{name}.m3u8")),
            name,
            tracks,
        });
        Some(self.playlists.len() - 1)
    }
}
