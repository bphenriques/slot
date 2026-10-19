//! Audiobooks as data: each book's place, speed and bookmarks on the shelf, its chapters, how far
//! through it is, and how far back it picks up after a pause. Playing them is `music::books`'.

pub(crate) mod chapters;
pub(crate) mod shelf;

pub use shelf::{Bookmark, Place, Shelf};

use crate::browse::BookList;
use crate::library::sort::sort_key;
use crate::library::{ArtistId, BookId, Library};

/// The speeds, as players offer them. X goes round from 0.75, so the slowest, seldom wanted,
/// is never in its way; the menu's Left reaches it.
pub(crate) const SPEEDS: [f32; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// How far back a book goes when the sleep timer stops it, in seconds.
pub(crate) const SLEPT_BACK: i32 = 120;

/// How far back a book picks up after `paused` seconds: nothing after a moment, the last
/// sentence after a while, more after hours.
pub(crate) fn rewind(paused: u64) -> f64 {
    match paused {
        0..5 => 0.0,
        5..300 => 5.0,
        300..3600 => 15.0,
        _ => 30.0,
    }
}

/// The books `list` holds, in its order; the library has them by title.
pub(crate) fn listed(lib: &Library, shelf: &Shelf, list: BookList) -> Vec<BookId> {
    let mut books: Vec<BookId> = (0..lib.books.len()).map(BookId).collect();
    let place = |b| shelf.place(lib, b);
    match list {
        BookList::Listening => books.retain(|&b| place(b).when > 0 && !place(b).finished),
        BookList::Finished => books.retain(|&b| place(b).finished),
        BookList::All => return books,
    }
    books.sort_by_key(|&b| std::cmp::Reverse(place(b).when));
    books
}

/// The book listened to last, unless it was finished.
pub(crate) fn latest(lib: &Library, shelf: &Shelf) -> Option<BookId> {
    listed(lib, shelf, BookList::Listening).first().copied()
}

/// Who wrote the books, by name.
pub(crate) fn authors(lib: &Library) -> Vec<ArtistId> {
    let mut authors: Vec<ArtistId> = lib
        .books
        .iter()
        .map(|b| lib.albums[b.album].artist)
        .collect();
    // By name, and by who for names alike, so each author's books are together to dedup.
    authors.sort_by_cached_key(|&a| (sort_key(&lib.artists[a].name), a));
    authors.dedup();
    authors
}

/// An author's books, by title.
pub(crate) fn by(lib: &Library, author: ArtistId) -> Vec<BookId> {
    (0..lib.books.len())
        .map(BookId)
        .filter(|&b| lib.albums[lib.books[b].album].artist == author)
        .collect()
}

/// Seconds into `book`, `pos` into its `part`, by its parts' lengths.
pub fn into_book(lib: &Library, book: BookId, part: usize, pos: f64) -> f64 {
    let parts = &lib.albums[lib.books[book].album].tracks;
    let before = parts[..part.min(parts.len())].iter();
    before.map(|&t| lib.tracks[t].secs).sum::<f64>() + pos
}

/// How far through `book`, 0 to 1, at `place`: by its parts' lengths, or by its parts alone
/// when a length is not known.
pub fn progress(lib: &Library, book: BookId, place: &Place) -> f64 {
    let b = &lib.books[book];
    let parts = &lib.albums[b.album].tracks;
    let done = if b.secs > 0.0 {
        into_book(lib, book, place.part, place.pos) / b.secs
    } else {
        place.part as f64 / parts.len().max(1) as f64
    };
    done.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::cycle;

    #[test]
    fn a_book_picks_up_further_back_the_longer_it_was_paused() {
        let back: Vec<f64> = [2, 60, 1200, 86400].map(rewind).into();
        assert_eq!(back, [0.0, 5.0, 15.0, 30.0]);
    }

    #[test]
    fn x_goes_round_the_usual_speeds_and_left_reaches_half() {
        assert_eq!(cycle(&SPEEDS[1..], 2.0, 0), 0.75, "X round again");
        assert_eq!(cycle(&SPEEDS[1..], 0.5, 0), 1.0, "from the slowest, normal");
        assert_eq!(cycle(&SPEEDS, 0.75, -1), 0.5, "Left reaches the slowest");
        assert_eq!(cycle(&SPEEDS, 0.5, -1), 0.5, "and stops there");
    }
}
