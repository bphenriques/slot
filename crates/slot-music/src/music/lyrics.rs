//! The playing song's lyrics, read as it shows on Now Playing or the lyrics: an `.lrc` beside it
//! or its tags, small enough to read at once, once a song; and where in them the song is.

use super::Music;
use crate::browse::{LyricsAt, View};
use crate::library::TrackId;
use crate::lyrics::{self, Lyrics};

impl Music {
    /// Reads the playing song's lyrics on Now Playing or the lyrics, when the song is new to
    /// them, so X shows only where there are some; a new song's are shown from the top. A book
    /// has none.
    pub(crate) fn read_lyrics(&mut self) {
        let view = self.browser.view();
        let shown = matches!(view, View::Lyrics | View::NowPlaying) && self.book().is_none();
        let Some(t) = self.player.current().filter(|_| shown) else {
            return;
        };
        if self.lyrics.as_ref().is_some_and(|(read, _)| *read == t) {
            return;
        }
        self.lyrics = Some((t, lyrics::read(&self.library.tracks[t].path)));
        self.browser.set_at(0);
        self.browser.scrolled = None;
    }

    pub(crate) fn lyrics_now(&self) -> Option<&Lyrics> {
        let (read, lyrics) = self.lyrics.as_ref()?;
        (Some(*read) == self.player.current()).then_some(lyrics.as_ref()?)
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

/// The song whose lyrics were read last, and what it has: none for a song without lyrics.
pub(crate) type SongLyrics = Option<(TrackId, Option<Lyrics>)>;
