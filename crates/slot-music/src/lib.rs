//! slot's music and audiobook player: everything it does, and every screen as data, with no
//! drawing. `Music` is what slot holds; the rest is what it is made of.

pub mod books;
mod browse;
mod library;
pub mod lyrics;
mod music;
mod playback;
mod screens;
mod settings;
pub mod store;
pub mod text;

pub use books::Chapter;
pub use browse::keys::{Mode, Press};
pub use browse::rows::{rows, Info, Row, RowIcon, RowText};
pub use browse::{
    per_page, Browser, Context, Key, LyricsAt, Outcome, Page, QueueView, Shuffle, View, MENU_ROWS,
};
pub use library::covers::Art;
pub use library::playlists::Playlist;
pub use library::scan::{cached as cached_library, scan as scan_library, Progress, CACHE_FILE};
pub use library::tags::Tags;
pub use library::{
    track_number, track_title, Album, AlbumId, Artist, ArtistId, Book, BookId, Library, Record,
    Track, TrackId, BOOKS_DIR, MUSIC_DIR,
};
pub use music::Music;
pub use playback::player::{ChapterAt, Player, Repeat};
pub use playback::queue::Queue;
pub use playback::recent::Recent;
pub use screens::{
    Body, Legend, ListBody, MainHold, NowPlayingBody, Screen, Status, Target, TextBody,
};
pub use settings::{cycle, ListSize, Setting, Settings, Theme};
pub use slot_player::{Command, Event, Handle, PlayState, Status as PlayerStatus};

/// Lowers this thread's priority (Linux niceness is per thread), so the player and the UI go
/// first.
pub(crate) fn background() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, libc::gettid() as libc::id_t, 10);
    }
}
