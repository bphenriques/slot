//! The song's lyrics, full screen, from Now Playing with X, which closes them too. Timed lyrics
//! follow the song, the line sung lit in the middle; Up and Down scroll, and a while after,
//! they follow again. Plain lyrics are read with Up and Down. The rest of the keys are Now
//! Playing's.

use std::time::{Duration, Instant};

use super::{now_playing, Body, Legend, Screen, TextBody};
use crate::browse::keys::Mode;
use crate::browse::{Browser, Context, Key, LyricsAt, Outcome};

use crate::{text, Music};
use slot_player::PlayState;

/// How long lyrics scrolled by hand stay where they were left before they follow the song.
const FOLLOW_AFTER: Duration = Duration::from_secs(4);

/// Up and Down scroll, on and on held; the rest act as on Now Playing.
pub(crate) fn mode(key: Key) -> Mode {
    match key {
        Key::Up | Key::Down => Mode::Repeat,
        Key::X => Mode::Once,
        key => now_playing::mode(key),
    }
}

pub(crate) fn held(key: Key) -> Outcome {
    now_playing::held(key)
}

pub(crate) fn press(b: &mut Browser, cx: &Context, key: Key) -> Outcome {
    match key {
        Key::X => {
            b.pop();
            Outcome::None
        }
        // Scrolled to a line of timed lyrics, A plays from it, as Apple Music and Spotify do.
        Key::A if cx.lyrics.timed && !following(b) => {
            b.scrolled = None;
            Outcome::FromLine(b.at())
        }
        Key::Up | Key::Down => {
            let LyricsAt { lines, sung, .. } = cx.lyrics;
            if lines == 0 {
                return Outcome::None;
            }
            // From the line in the middle: the one sung, unless scrolled to another.
            let from = match following(b) {
                true => sung.unwrap_or(0),
                false => b.at(),
            };
            let to = match key {
                Key::Up => from.saturating_sub(1),
                _ => (from + 1).min(lines - 1),
            };
            b.set_at(to);
            b.scrolled = Some(Instant::now());
            Outcome::None
        }
        key => now_playing::press(b, cx, key),
    }
}

/// Whether the lyrics follow the song: not scrolled by hand a moment ago.
fn following(b: &Browser) -> bool {
    b.scrolled.is_none_or(|at| at.elapsed() >= FOLLOW_AFTER)
}

pub(crate) fn screen(music: &Music) -> Screen {
    let title = match music.player.current() {
        Some(t) => music.library.tracks[t].title.clone(),
        None => return Screen::message(text::LYRICS, text::NOTHING_PLAYING),
    };
    let Some(lyrics) = music.lyrics_now() else {
        return Screen::message(&title, text::NO_LYRICS);
    };
    let LyricsAt { sung, .. } = music.lyrics_at();
    let focus = match following(&music.browser) && lyrics.timed() {
        true => sung.unwrap_or(0),
        false => music.browser.at(),
    };
    let body = TextBody {
        lines: lyrics.lines.iter().map(|l| l.text.clone()).collect(),
        // Timed, the line sung is lit and the rest dim; plain, all is to be read.
        lit: lyrics.timed().then_some(sung),
        focus,
    };
    let playing = music.player.state() == PlayState::Playing;
    let line = lyrics.timed() && !following(&music.browser);
    Screen::new(title, Legend::Lyrics { playing, line }, Body::Text(body))
}
