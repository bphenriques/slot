//! The song's lyrics, full screen, from Now Playing with X, which closes them too. Timed lyrics
//! follow the song, the line sung lit in the middle; Up and Down scroll, and a while after,
//! they follow again. Plain lyrics are read with Up and Down. The rest of the keys are Now
//! Playing's.

use std::time::{Duration, Instant};

use super::{now_playing, Body, Screen, TextBody};
use crate::browse::{Browser, Context, Key, LyricsAt, Outcome};

use crate::{text, Music};
use slot_player::PlayState;

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
            let from = if following(b, cx.now) {
                sung.unwrap_or(0)
            } else {
                b.at()
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

/// Seconds until timed lyrics show the song moved on, as `screen` shows them: the next line
/// sung, while it plays; and, scrolled by hand, when they follow it again.
pub(crate) fn steps_in(music: &Music) -> Option<f64> {
    let lyrics = music.lyrics_now().filter(|l| l.timed())?;
    let since = |at| music.now.at.saturating_duration_since(at);
    let back = music
        .browser
        .scrolled
        .map(|at| FOLLOW_AFTER.saturating_sub(since(at)));
    let back = back.filter(|b| !b.is_zero()).map(|b| b.as_secs_f64());
    let player = &music.player;
    let (at, rate) = (player.time().0, f64::from(player.speed()));
    let sung = lyrics.lines.iter().filter_map(|l| l.at).filter(|&t| t > at);
    let line = (player.state() == PlayState::Playing)
        .then(|| sung.reduce(f64::min).map(|t| (t - at) / rate))
        .flatten();
    [back, line].into_iter().flatten().reduce(f64::min)
}

pub(crate) fn screen(music: &Music) -> Screen {
    let title = match music.player.current() {
        Some(t) => {
            let track = &music.library.tracks[t];
            text::lyrics_of(&track.title, &track.artist)
        }
        None => return Screen::message(text::LYRICS, text::NOTHING_PLAYING),
    };
    let Some(lyrics) = music.lyrics_now() else {
        return Screen::message(&title, text::NO_LYRICS);
    };
    let LyricsAt { sung, .. } = music.lyrics_at();
    let focus = if following(&music.browser, music.now.at) && lyrics.timed() {
        sung.unwrap_or(0)
    } else {
        music.browser.at()
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
