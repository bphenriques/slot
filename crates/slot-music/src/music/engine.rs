//! The player as the music's shell sees it: told commands, read for its status, asked what
//! happened. slot-player's `Handle` is one; a test's own, which answers as the test says, is
//! another.

use slot_player::{Command, Event, Handle, Status};

pub(crate) trait Engine {
    fn send(&self, command: Command);
    fn status(&self) -> Status;
    /// What happened since last asked.
    fn events(&self) -> Vec<Event>;
}

impl Engine for Handle {
    fn send(&self, command: Command) {
        Handle::send(self, command);
    }

    fn status(&self) -> Status {
        Handle::status(self)
    }

    fn events(&self) -> Vec<Event> {
        Handle::events(self).collect()
    }
}

/// Starts a player: the device's, or a test's; again whenever the last is gone.
pub(crate) type Start = Box<dyn Fn() -> Box<dyn Engine>>;
