//! The playing song's lyrics, read by the worker as it shows on Now Playing or the lyrics, and
//! the next song's ahead of it, so they are there as it starts; and where in them the song is.

use super::worker::Job;
use super::Music;
use crate::browse::{LyricsAt, View};
use crate::lyrics::Lyrics;

impl Music {
    /// Asks for the playing song's lyrics, the next's and the one before's on Now Playing or the
    /// lyrics, each once, so X shows only where there are some, and a skip either way finds them
    /// read and its hints steady; a new song's are shown from the top. A book has none.
    pub(crate) fn read_lyrics(&mut self) {
        let view = self.browser.view();
        let shown = matches!(view, View::Lyrics | View::NowPlaying) && self.book().is_none();
        let Some(t) = self.player.current().filter(|_| shown) else {
            return;
        };
        if self.ahead.lyrics_for.replace(t) != Some(t) {
            self.browser.set_at(0);
            self.browser.scrolled = None;
        }
        let wanted: Vec<_> = [Some(t), self.player.up_next(), self.player.previous()]
            .into_iter()
            .flatten()
            .collect();
        for song in self.ahead.lyrics.want(&wanted) {
            let path = self.library.tracks[song].path.clone();
            self.work(Job::Lyrics { track: song, path });
        }
    }

    /// The song's lyrics as read: none until they are, then whether it has any.
    pub(crate) fn lyrics_read(&self) -> Option<Option<&Lyrics>> {
        let read = self.ahead.lyrics.of(self.player.current()?)?;
        Some(read.as_ref())
    }

    pub(crate) fn lyrics_now(&self) -> Option<&Lyrics> {
        self.ahead.lyrics.of(self.player.current()?)?.as_ref()
    }

    pub(crate) fn lyrics_at(&self) -> LyricsAt {
        let Some(lyrics) = self.lyrics_now() else {
            return LyricsAt::default();
        };
        LyricsAt {
            lines: lyrics.lines.len(),
            sung: lyrics.current(self.player.time().0),
            timed: lyrics.timed(),
        }
    }
}
