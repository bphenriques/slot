//! The queue, opened from Now Playing. A plays from a track, or held puts it next; X takes it
//! out, and held goes on taking out the ones after, quicker and quicker; Y puts it in Favorites
//! or takes it out, as in every list, and held lifts it to carry with Up and Down, as a long press
//! drags a row on a phone, until Y puts it down (R1 puts it next, B back).

use super::{list, Body, Screen};
use crate::browse::{Browser, Context, Key, Outcome, QueueView};
use crate::{text, Music};

/// Seconds the queue has left to play: the rest of the song playing and the songs after it,
/// at its speed.
pub(crate) fn left(music: &Music) -> f64 {
    let (lib, player) = (&music.library, &music.player);
    let queue = player.queue();
    let after = queue.tracks.iter().skip(queue.at + 1);
    let after: f64 = after.map(|&t| lib.tracks[t].secs).sum();
    let (at, len) = player.time();
    (after + (len - at).max(0.0)) / f64::from(player.speed())
}

/// Seconds until the queue's time left shows the next minute, while it plays.
pub(crate) fn steps_in(music: &Music) -> Option<f64> {
    let playing = music.player.state() == slot_player::PlayState::Playing;
    let rate = f64::from(music.player.speed()) / 60.0;
    playing
        .then(|| super::whole_in(-left(music) / 60.0, rate))
        .flatten()
}

/// After how many repeats X held takes out one track more at a time: about a second, at the
/// quickest repeat.
const RUN: usize = 20;

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
    if from < queue.at {
        queue.at
    } else {
        queue.at + 1
    }
    .min(queue.tracks.len().saturating_sub(1))
}

pub(crate) fn screen(music: &Music) -> Screen {
    if music.player.queue().tracks.is_empty() {
        return Screen::message(text::QUEUE, text::QUEUE_EMPTY);
    }
    let mut screen = list::rows_screen(music);
    // Beside where the cursor is, how long the queue has left, as players say under it.
    screen.corner = text::queue_corner(&screen.corner, left(music));
    // The carried track's highlight is lifted; its legend says how to put it down.
    if let (Some(_), Body::List(list)) = (music.browser.carry, &mut screen.body) {
        list.lifted = true;
    }
    screen
}
