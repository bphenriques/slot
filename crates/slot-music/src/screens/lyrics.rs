//! The song's lyrics, full screen, from Now Playing with X, which closes them too. Timed lyrics
//! follow the song, the line sung lit in the middle; Up and Down scroll, and a while after,
//! they follow again. Plain lyrics are read with Up and Down. The rest of the keys are Now
//! Playing's.

use std::time::{Duration, Instant};

use super::{now_playing, Body, Screen, TextBody};
use crate::browse::{Browser, Context, Key, LyricsAt, Outcome};

use crate::{text, Music};

/// How long lyrics scrolled by hand stay where they were left before they follow the song.
const FOLLOW_AFTER: Duration = Duration::from_secs(4);

/// Up and Down scroll, on and on held; the rest act as on Now Playing.
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
        Key::A if cx.lyrics.timed && !following(b, cx.now) => {
            b.scrolled = None;
            Outcome::FromLine(b.at())
        }
        Key::Up | Key::Down => {
            let LyricsAt { lines, sung, .. } = cx.lyrics;
            if lines == 0 {
                return Outcome::None;
            }
            // From the line in the middle: the one sung, unless scrolled to another.
            let from = match following(b, cx.now) {
                true => sung.unwrap_or(0),
                false => b.at(),
            };
            let to = match key {
                Key::Up => from.saturating_sub(1),
                _ => (from + 1).min(lines - 1),
            };
            b.set_at(to);
            b.scrolled = Some(cx.now);
            Outcome::None
        }
        key => now_playing::press(b, cx, key),
    }
}

/// Whether the lyrics follow the song `now`: not scrolled by hand a moment ago.
pub(crate) fn following(b: &Browser, now: Instant) -> bool {
    b.scrolled
        .is_none_or(|at| now.saturating_duration_since(at) >= FOLLOW_AFTER)
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
    let focus = match following(&music.browser, music.now.at) && lyrics.timed() {
        true => sung.unwrap_or(0),
        false => music.browser.at(),
    };
    let body = TextBody {
        lines: lyrics.lines.iter().map(|l| l.text.clone()).collect(),
        // Timed, the line sung is lit and the rest dim; plain, all is to be read.
        lit: lyrics.timed().then_some(sung),
        focus,
    };
    let legend = super::legend(&music.browser, &music.context());
    Screen::new(title, legend, Body::Text(body))
}
