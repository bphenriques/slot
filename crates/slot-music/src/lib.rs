//! The music and audiobook player's behaviour, without drawing: what slot sees of it is its face,
//! `Music`. Buttons go in as `Input`; each frame a `Screen` comes out, plain data the kit draws,
//! with the album picture (`Art`) and how strong the motor should be. All else is the crate's own.

mod books;
mod browse;
mod library;
mod lyrics;
mod music;
mod playback;
mod screens;
mod settings;
mod store;
pub mod text;

pub use browse::rows::{RowIcon, RowText};
pub use library::covers::Art;
pub use library::AlbumId;
pub use music::{Button, Input, Music};
pub use playback::player::Repeat;
pub use screens::{
    Body, Hold, Legend, ListBody, MainHold, NowPlayingBody, Screen, Status, Target, TextBody,
};
pub use settings::Theme;

// What the tests of the parts reach for, from the crate's root.
#[cfg(test)]
pub(crate) use {
    browse::keys::Mode,
    browse::rows::{rows, Row},
    browse::{BookList, Browser, Context, Key, LyricsAt, Outcome, Page, QueueView, Shuffle, View},
    library::scan::{cached as cached_library, scan as scan_library, Progress, CACHE_FILE},
    library::tags::Tags,
    library::{track_number, track_title, BookId, Library, Record, TrackId},
    playback::player::Player,
    playback::queue::Queue,
    settings::{cycle, ListSize, Setting, Settings},
    slot_player::{Command, Event, PlayState, Status as PlayerStatus},
};

#[cfg(test)]
mod tests;

pub(crate) fn background() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}
