//! A player of files, and the words slot and a player speak: slot sends a [`Command`] and
//! hears an [`Event`]. A player knows files and transport and nothing else; what an album or a
//! book is, and what plays next, are slot's.
//!
//! [`Handle`] is the player's face: decoding with symphonia into ALSA, or into any [`Output`].

mod alsa;
#[cfg(test)]
mod bench;
mod decode;
mod engine;
mod eq;
mod level;
mod stretch;
mod track;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

pub use engine::Output;
pub use eq::Band;
pub use level::Leveling;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum PlayState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// What slot asks of a player.
#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    /// Open and play `path`, from `at` seconds in, at `speed` (1 is as recorded).
    Play {
        path: PathBuf,
        at: f64,
        speed: f32,
    },
    /// As `Play`, but paused there: a song skipped to while paused waits to be played.
    Cue {
        path: PathBuf,
        at: f64,
        speed: f32,
    },
    Pause,
    Resume,
    Stop,
    Seek(f64),
    /// How fast to play, 0.5 to 2, the voice kept at its pitch.
    Speed(f32),
    /// Both sides alike, or each its own.
    Mono(bool),
    /// How loud to play each file, from its ReplayGain.
    Level(Leveling),
    /// What to play straight after the file playing, with no gap, when it runs out by itself;
    /// none to play nothing after it. A file played anew forgets it.
    Next(Option<PathBuf>),
    /// Over how many seconds the end of a song fades into the next; 0 for none.
    Crossfade(f64),
    /// Skips what goes on past a moment of silence: for books.
    SkipSilence(bool),
    /// How many times louder to play, the loudest rounded off rather than clipped: for books.
    Boost(f32),
    /// The equalizer's bands, after the files are mixed; none plays as recorded.
    Equalizer(Vec<Band>),
}

/// What a player tells slot.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    /// The player stopped working, and plays no more: it could not start, or it failed.
    Gone,
    /// The file ran out by itself.
    End(PathBuf),
    /// The file ran out, and play went on with no gap into `Next`'s, which is now playing.
    Into(PathBuf),
    /// The file could not be played: slot moves on to the next.
    Error(String),
    /// The sound device stopped working, not the file: the player has paused where it was.
    Lost(String),
}

/// A player running on its own thread: commands in, events out. A player that cannot start, or
/// that panics, says so with `Gone`, and drops what it is sent.
pub struct Handle {
    commands: Sender<engine::In>,
    events: Receiver<Event>,
    shared: Arc<engine::Shared>,
}

/// The player as it is now: playing, paused or stopped, and where in the file playing, and how
/// long it is, in seconds; read whenever wanted, rather than told.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Status {
    pub state: PlayState,
    pub at: f64,
    pub len: f64,
    /// Whether the device may still be heard: from the first write until it is let go, its
    /// last sound faded out. Paused, the state says so at once; this, once it is quiet.
    pub sounding: bool,
    /// What the writer has done since it began.
    pub work: Work,
}

/// The writer's work since it began, in nanoseconds of its own thread's time, never its waiting:
/// decoding the files, mixing them to what plays, and writing that to the device, of which
/// `library` in the device's own library (ALSA's), the rest the output's own. The performance
/// log tells it as shares of a core, to see where the player's goes on the device.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Work {
    pub decode: u64,
    pub mix: u64,
    pub write: u64,
    pub library: u64,
}

impl Handle {
    /// Runs `player` on a thread named `name`, with the two ends it talks through and the status
    /// it keeps.
    fn spawn(
        name: &str,
        player: impl FnOnce(Arc<engine::Shared>, Receiver<engine::In>, Sender<engine::In>, Sender<Event>)
            + Send
            + 'static,
    ) -> Handle {
        let shared = Arc::new(engine::Shared::default());
        let kept = shared.clone();
        let (commands, command_rx) = mpsc::channel();
        let back = commands.clone();
        let (event_tx, events) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let told = event_tx.clone();
                let run = std::panic::AssertUnwindSafe(|| player(kept, command_rx, back, event_tx));
                if std::panic::catch_unwind(run).is_err() {
                    let _ = told.send(Event::Gone);
                }
            });
        if let Err(e) = spawned {
            log::warn!("{name}: {e}");
        }
        Handle {
            commands,
            events,
            shared,
        }
    }

    /// slot's own player, on `output`.
    pub fn on(output: impl Output) -> Handle {
        Handle::spawn("slot-player", move |shared, commands, back, events| {
            engine::run(output, shared, &commands, back, &events)
        })
    }

    /// slot's own player, on the device's ALSA output.
    pub fn slot() -> Handle {
        Handle::spawn(
            "slot-player",
            |shared, commands, back, events| match alsa::Alsa::load() {
                Ok(alsa) => engine::run(alsa, shared, &commands, back, &events),
                Err(why) => {
                    log::warn!("{why}");
                    let _ = events.send(Event::Gone);
                }
            },
        )
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(engine::In::Command(command));
    }

    /// The player as it is now.
    pub fn status(&self) -> Status {
        engine::status(&self.shared)
    }

    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }

    /// The next event, waiting up to `timeout` for one: for tests.
    pub fn wait(&self, timeout: std::time::Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }
}

/// Let go, the player stops: its threads end, the device with them. The writer holds an end of
/// the engine's inbox too, so the inbox closing is not enough to say so.
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.commands.send(engine::In::Quit);
    }
}
