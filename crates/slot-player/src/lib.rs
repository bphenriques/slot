//! A player of files, and the words slot and a player speak: slot sends a [`Command`] and
//! hears an [`Event`]. A player knows files and transport and nothing else; what an album or a
//! book is, and what plays next, are slot's.
//!
//! [`Handle`] is the one face of every player: slot's own, here, decoding with rodio into ALSA,
//! or another, as plorpOS's Muse is, over its socket.

mod alsa;
mod engine;
mod level;
mod stretch;
mod track;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

pub use engine::{Capture, Output};
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
}

/// What a player tells slot.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    /// The player is there to play, or went away.
    Connected(bool),
    State(PlayState),
    /// Where the file playing is, and how long it is, in seconds.
    Pos {
        at: f64,
        len: f64,
    },
    /// The file ran out by itself.
    End(PathBuf),
    /// The file ran out, and play went on with no gap into `Next`'s, which is now playing.
    Into(PathBuf),
    Error(String),
    /// A file was opened: its chapters follow, each where it starts and its title.
    Meta,
    Chapter {
        at: f64,
        title: String,
    },
}

/// A player running on its own thread: commands in, events out. A player that cannot start
/// says so with `Connected(false)`, and drops what it is sent.
pub struct Handle {
    commands: Sender<Command>,
    events: Receiver<Event>,
}

impl Handle {
    /// Runs `player` on a thread named `name`, with the two ends it talks through.
    pub fn spawn(
        name: &str,
        player: impl FnOnce(Receiver<Command>, Sender<Event>) + Send + 'static,
    ) -> Handle {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || player(command_rx, event_tx));
        if let Err(e) = spawned {
            eprintln!("slot: {name}: {e}");
        }
        Handle { commands, events }
    }

    /// slot's own player, on `output`.
    pub fn on(output: impl Output) -> Handle {
        Handle::spawn("slot-player", move |commands, events| {
            engine::run(output, &commands, &events)
        })
    }

    /// slot's own player, on the device's ALSA output.
    pub fn slot() -> Handle {
        Handle::spawn("slot-player", |commands, events| match alsa::Alsa::load() {
            Ok(alsa) => engine::run(alsa, &commands, &events),
            Err(why) => {
                eprintln!("slot: player: {why}");
                let _ = events.send(Event::Connected(false));
            }
        })
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }

    /// The next event, waiting up to `timeout` for one: for tests.
    pub fn wait(&self, timeout: std::time::Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }
}
