//! Audiobooks: their list, places, keys, speed and settings.

use super::common::*;

#[test]
fn a_book_s_chapters_skip_within_it_and_its_speed_goes_to_the_player_but_not_music_s() {
    let (_d, lib) = library(&[
        "../Audiobooks/Wilde - Tales/01 one.m4b",
        "../Audiobooks/Wilde - Tales/02 two.m4b",
        "X/A/01 song.mp3",
    ]);
    let song = lib.tracks_by_title[0];
    let mut p = Player::default();
    p.set_speed(1.5);
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    let play = p.play(&lib, Queue::new(parts, 0, None)).unwrap();
    assert!(matches!(play, Command::Play { speed: 1.5, .. }), "{play:?}");
    // As read from the file when it opens.
    let marks = [(0.0, "One"), (60.0, "Two"), (120.0, "Three")];
    let part = p.current().unwrap();
    p.set_chapters(part, chapters(&marks));
    heard(&mut p, 70.0, 180.0);
    let c = p.chapter().unwrap();
    let at = (c.index, c.chapter.title.as_str(), c.chapter.start, c.end);
    assert_eq!(at, (1, "Two", 60.0, 120.0));
    assert_eq!(
        p.skip(&lib, 1),
        Some(Command::Seek(120.0)),
        "the next chapter"
    );
    heard(&mut p, 120.5, 180.0);
    assert_eq!(
        p.skip(&lib, -1),
        Some(Command::Seek(60.0)),
        "just in: the one before"
    );
    // There, and on a while.
    heard(&mut p, 60.0, 180.0);
    heard(&mut p, 150.0, 180.0);
    assert_eq!(
        p.skip(&lib, -1),
        Some(Command::Seek(120.0)),
        "well in: its start"
    );
    // Past the last chapter, the next file.
    assert!(matches!(p.skip(&lib, 1), Some(Command::Play { .. })));
    assert_eq!(p.chapter(), None, "a new file's chapters are its own");
    // Music plays as recorded whatever speed the book left.
    let music = p.play(&lib, Queue::new(vec![song], 0, None)).unwrap();
    assert!(
        matches!(music, Command::Play { speed: 1.0, .. }),
        "{music:?}"
    );
}

#[test]
fn a_book_says_its_speed_how_far_through_it_is_and_its_time_left() {
    use slot_music::text::book_left;
    assert_eq!(
        book_left(1.25, Some(0.374), 3723.0),
        "1.25× · 37% · 1:02:03 left"
    );
    assert_eq!(
        book_left(1.0, None, 90.0),
        "1× · 1:30 left",
        "parts of unknown length"
    );
}

#[test]
fn audiobooks_are_kept_apart_from_the_music_and_open_from_the_menu() {
    let (_d, lib) = library(&[
        "X/A/01 one.mp3",
        "../Audiobooks/Kipling - Just So/01 whale.mp3",
        "../Audiobooks/Kipling - Just So/02 camel.mp3",
        "../Audiobooks/Wilde - The Happy Prince.m4b",
    ]);
    assert_eq!((lib.music_artists.len(), lib.music_tracks().len()), (1, 1));
    assert_eq!(lib.albums_by_name.len(), 1, "books are not albums");
    let mut books: Vec<(&str, &str, usize)> = lib
        .books
        .iter()
        .map(|b| {
            let book = &lib.albums[b.album];
            let author = lib.artists[book.artist].name.as_str();
            (book.name.as_str(), author, book.tracks.len())
        })
        .collect();
    books.sort();
    assert_eq!(
        books,
        [("Just So", "Kipling", 2), ("The Happy Prince", "Wilde", 1)]
    );
    let mut b = Browser::default();
    // Shuffle Library, Playlists, then Audiobooks.
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    assert_eq!(b.view(), View::Books);
    // The first by title, as none is begun.
    let book = BookId(0);
    assert_eq!(
        press(&mut b, &lib, &[Key::A]),
        Outcome::PlayBook { book, over: false }
    );
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn books_list_the_one_listened_to_first_with_how_far_by_the_parts_lengths() {
    use slot_music::books::{Place, Shelf};
    let (_d, lib) = books();
    let mut shelf = Shelf::new();
    let place = Place {
        part: 1,
        pos: 60.0,
        when: 1_700_000_000,
        ..Place::default()
    };
    shelf.set_place(&lib, BookId(1), Place { when: 1, ..place });
    shelf.set_place(&lib, BookId(0), place);
    let cx = Context {
        shelf: &shelf,
        ..Context::new(&lib)
    };
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Down, Key::Down, Key::A]);
    let page = b.page(&cx);
    assert_eq!(page.rows[0].text, "Just So", "the latest listened to");
    // A minute into the second part, after a minute-long first: two of four minutes.
    assert_eq!(page.values[0].as_deref(), Some("50%"));
    assert_eq!(page.rows[1].text, "The Happy Prince");
}

#[test]
fn a_book_s_keys_jump_mark_and_change_speed_and_list_its_marks_and_chapters() {
    let (_d, lib) = books();
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    let chapters = chapters(&[(0.0, "Whale"), (30.0, "Throat")]);
    let queue = QueueView {
        tracks: &parts,
        at: 0,
        playing: true,
        waiting: false,
        chapters: &chapters,
        chapter: Some(1),
        speed: 1.0,
    };
    let cx = Context {
        queue,
        book: Some(BookId(0)),
        ..Context::new(&lib)
    };
    let mut b = Browser::default();
    b.press(&cx, Key::Select);
    assert_eq!(b.view(), View::NowPlaying);
    assert_eq!(b.press(&cx, Key::Left), Outcome::Seek(-30));
    let fifteen = Context {
        settings: Settings {
            jump: 15,
            ..Settings::default()
        },
        ..cx
    };
    assert_eq!(
        b.press(&fifteen, Key::Right),
        Outcome::Seek(15),
        "Jump Length"
    );
    assert_eq!(b.press(&cx, Key::R1), Outcome::Skip(1), "the next chapter");
    assert_eq!(b.press(&cx, Key::X), Outcome::Speed(0), "round the speeds");
    assert_eq!(b.press(&cx, Key::Y), Outcome::Bookmark);
    // Y held lists the marks, and B comes back.
    assert_eq!(b.mode(&cx, Key::Y), Mode::Hold);
    b.held(&cx, Key::Y);
    assert_eq!(b.view(), View::Bookmarks(BookId(0)));
    b.press(&cx, Key::B);
    assert_eq!(b.view(), View::NowPlaying);
    b.press(&cx, Key::Select);
    assert_eq!(b.view(), View::Chapters);
    let page = b.page(&cx);
    assert_eq!(page.selected, 1, "on the one playing");
    assert!(page.rows[0].faint && page.rows[1].icon.is_some());
    assert_eq!(page.values[0].as_deref(), Some("0:30"), "how long each is");
    assert_eq!(
        page.values[1].as_deref(),
        Some("0:30"),
        "the last, to its part's end"
    );
    b.press(&cx, Key::Up);
    assert_eq!(b.press(&cx, Key::A), Outcome::Chapter(0));
    assert_eq!(b.view(), View::NowPlaying, "back to it");
    // A slip can be undone from the menu, which offers it first.
    let cx = Context {
        back: Some(754.0),
        ..cx
    };
    b.press(&cx, Key::Start);
    let page = b.page(&cx);
    assert_eq!(page.rows[0].text, "Back to 12:34");
    assert_eq!(b.press(&cx, Key::A), Outcome::Back);
}

#[test]
fn a_library_with_books_has_their_settings_on_a_page_of_their_own() {
    let (_d, lib) = books();
    let mut b = Browser::default();
    let cx_own = cx(&lib, QueueView::default(), Settings::default());
    // Up from the first row goes round to Settings, the last.
    for key in [Key::Up, Key::A] {
        b.press(&cx_own, key);
    }
    let page = b.page(&cx_own);
    let row = page
        .rows
        .iter()
        .position(|r| r.text == "Audiobooks")
        .expect("Audiobooks");
    assert!(page.rows[row].opens);
    for _ in 0..row {
        b.press(&cx_own, Key::Down);
    }
    b.press(&cx_own, Key::A);
    assert_eq!(b.view(), View::BookSettings);
    let rows: Vec<String> = b
        .page(&cx_own)
        .rows
        .iter()
        .map(|r| r.text.clone())
        .collect();
    assert_eq!(rows, ["Jump Length", "Skip Silence", "Volume Boost"]);
}
