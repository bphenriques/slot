//! The queue, opened from Now Playing. A plays from a track, or held puts it next; X takes it
//! out, and held goes on taking out the ones after, quicker and quicker; Y puts it in Favorites
//! or takes it out, as in every list, and held lifts it to carry with Up and Down, as a long press
//! drags a row on a phone, until Y puts it down (R1 puts it next, B back).

use super::{list, Body, Screen};
use crate::browse::{Browser, Context, Key, Outcome, QueueView};
use crate::{text, Music};

/// After how many repeats X held takes out one track more at a time: about a second, at the
/// quickest repeat.
const RUN: usize = 20;

/// A and Y wait to see whether they are held; A not on the track playing, which it starts over
/// at once. X repeats, as the moving keys do.
/// The keys a queued track takes tapped; the rest move through the list.
pub(crate) fn press(b: &mut Browser, cx: &Context, i: usize, key: Key) -> Option<Outcome> {
    match key {
        Key::X => {
            b.run = 0;
            Some(Outcome::Remove { at: i, n: 1 })
        }
        Key::Y => Some(Outcome::Favorite(cx.queue.tracks[i])),
        _ => None,
    }
}

/// Held: A puts the track next, Y lifts it to carry, and X goes on taking tracks out, one more
/// at once each `RUN` repeats.
pub(crate) fn held(b: &mut Browser, cx: &Context, i: usize, key: Key) -> Outcome {
    match key {
        Key::A => {
            // The highlight goes with the track.
            let to = next_place(cx.queue, i);
            b.set_at(to);
            Outcome::Move(i, to)
        }
        Key::X => {
            b.run += 1;
            Outcome::Remove {
                at: i,
                n: 1 + b.run / RUN,
            }
        }
        Key::Y => {
            b.carry = Some(i);
            Outcome::None
        }
        _ => Outcome::None,
    }
}

/// The keys while a track is carried: every other key waits until it is put down.
pub(crate) fn carrying(b: &mut Browser, cx: &Context, from: usize, key: Key) -> Outcome {
    let i = b.at();
    let n = cx.queue.tracks.len();
    let to = match key {
        Key::Up if i > 0 => i - 1,
        Key::Down if i + 1 < n => i + 1,
        Key::R1 => next_place(cx.queue, i),
        Key::B => from,
        Key::Y => i,
        _ => return Outcome::None,
    };
    if !matches!(key, Key::Up | Key::Down) {
        b.carry = None;
    }
    b.set_at(to);
    Outcome::Move(i, to)
}

/// Where the track at `from` lands to play next: just after the one playing.
fn next_place(queue: QueueView, from: usize) -> usize {
    match from < queue.at {
        true => queue.at,
        false => queue.at + 1,
    }
    .min(queue.tracks.len().saturating_sub(1))
}

pub(crate) fn screen(music: &Music) -> Screen {
    if music.player.queue().tracks.is_empty() {
        return Screen::message(text::QUEUE, text::QUEUE_EMPTY);
    }
    // A on the track playing starts it over, and says so.
    let _at = music.player.queue().at;
    let mut screen = list::rows_screen(music);
    // The carried track's highlight is lifted; its legend says how to put it down.
    if let (Some(_), Body::List(list)) = (music.browser.carry, &mut screen.body) {
        list.lifted = true;
    }
    screen
}
