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
fn a_book_s_time_left_is_said_in_hours_and_minutes_as_the_sleep_timer_says_them() {
    use crate::text::time_left;
    assert_eq!(
        time_left(10_740.0),
        "2 h 59 min left",
        "not 2:59, which reads as minutes"
    );
    assert_eq!(
        time_left(3723.0),
        "1 h 3 min left",
        "a minute begun counts whole"
    );
    assert_eq!(time_left(7200.0), "2 h left");
    assert_eq!(time_left(90.0), "2 min left");
    assert_eq!(
        time_left(1.0),
        "1 min left",
        "never none while some is left"
    );
}

#[test]
fn audiobooks_are_kept_apart_from_the_music_and_open_from_the_menu() {
    let (_d, lib) = library(&[
        "X/A/01 one.mp3",
        "../Audiobooks/Kipling - Just So/01 whale.mp3",
        "../Audiobooks/Kipling - Just So/02 camel.mp3",
        "../Audiobooks/Wilde - The Selfish Giant.m4b",
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
        [("Just So", "Kipling", 2), ("The Selfish Giant", "Wilde", 1)]
    );
    let mut b = Browser::default();
    // Shuffle Library, Library, Playlists, then Audiobooks.
    press(
        &mut b,
        &lib,
        &[Key::Down, Key::Down, Key::Down, Key::Down, Key::A],
    );
    assert_eq!(b.view(), View::Books);
    let page = b.page(&Context::new(&lib, at(0)));
    let rows: Vec<&str> = page.rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(
        rows,
        ["Authors", "All Books"],
        "none begun, nothing to continue"
    );
    // The first by title.
    let book = BookId(0);
    assert_eq!(
        press(&mut b, &lib, &[Key::Down, Key::A, Key::A]),
        Outcome::PlayBook { book, over: false }
    );
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn the_audiobooks_menu_continues_the_latest_and_lists_them_by_progress_and_author() {
    use crate::books::{Place, Shelf};
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
        ..Context::new(&lib, at(0))
    };
    let mut b = Browser::default();
    press(
        &mut b,
        &lib,
        &[Key::Down, Key::Down, Key::Down, Key::Down, Key::A],
    );
    let page = b.page(&cx);
    assert_eq!(
        page.values[0].as_deref(),
        Some("Just So"),
        "the book beside Continue"
    );
    let rows: Vec<(&str, &str)> = page
        .rows
        .iter()
        .map(|r| (r.text.as_str(), r.after.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Continue", ""),
            ("In Progress", ""),
            ("Authors", ""),
            ("All Books", "")
        ],
        "nothing finished yet"
    );
    assert_eq!(page.values[1].as_deref(), Some("2"));
    // In Progress: the latest listened to first.
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    assert_eq!(b.view(), View::BookList(crate::BookList::Listening));
    let page = b.page(&cx);
    assert_eq!(page.rows[0].text, "Just So", "the latest listened to");
    // A minute into the second part, after a minute-long first: two of four minutes.
    assert_eq!(page.values[0].as_deref(), Some("50%"));
    assert_eq!(page.rows[1].text, "The Selfish Giant");
    // Authors, each with their books.
    b.press(&cx, Key::B);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    let page = b.page(&cx);
    let authors: Vec<&str> = page.rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(authors, ["Kipling", "Wilde"]);
    b.press(&cx, Key::Down);
    b.press(&cx, Key::A);
    assert_eq!(b.page(&cx).rows[0].text, "The Selfish Giant");
    // Continue plays on the latest.
    b.press(&cx, Key::B);
    b.press(&cx, Key::B);
    b.press(&cx, Key::Up);
    b.press(&cx, Key::Up);
    assert_eq!(
        b.press(&cx, Key::A),
        Outcome::PlayBook {
            book: BookId(0),
            over: false
        }
    );
    assert_eq!(b.view(), View::NowPlaying);
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
        speed: 1.0,
    };
    // The first part's chapters, read; it was left 40 s in, in the second.
    let mut read = crate::music::ReadAhead::default();
    read.want(&parts[..1]);
    read.put(parts[0], chapters);
    let mut shelf = crate::books::Shelf::new();
    let left = crate::books::Place {
        pos: 40.0,
        when: 1,
        ..Default::default()
    };
    shelf.set_place(&lib, BookId(0), left);
    let cx = Context {
        queue,
        book: Some(BookId(0)),
        chapters: &read,
        shelf: &shelf,
        ..Context::new(&lib, at(0))
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
    assert_eq!(b.view(), View::BookChapters(BookId(0)));
    let page = b.page(&cx);
    assert_eq!(page.selected, 1, "on the one playing");
    assert!(page.rows[0].faint && page.rows[1].playing);
    assert_eq!(page.values[0].as_deref(), Some("0:30"), "how long each is");
    assert_eq!(
        page.values[1].as_deref(),
        Some("0:30"),
        "the last, to its part's end"
    );
    b.press(&cx, Key::Up);
    let first = Outcome::BookChapter {
        book: BookId(0),
        part: 0,
        chapter: Some(0),
    };
    assert_eq!(b.press(&cx, Key::A), first);
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
    assert_eq!(rows, ["Jump Length", "Volume Boost"]);
}

#[test]
fn a_finished_book_leaves_in_progress_for_finished_last_on_the_menu() {
    use crate::books::{Place, Shelf};
    let (_d, lib) = books();
    let mut shelf = Shelf::new();
    let begun = Place {
        when: 1_700_000_000,
        ..Place::default()
    };
    shelf.set_place(&lib, BookId(0), begun);
    shelf.set_place(
        &lib,
        BookId(1),
        Place {
            finished: true,
            ..begun
        },
    );
    let cx = Context {
        shelf: &shelf,
        ..Context::new(&lib, at(0))
    };
    let mut b = Browser::default();
    press(
        &mut b,
        &lib,
        &[Key::Down, Key::Down, Key::Down, Key::Down, Key::A],
    );
    let page = b.page(&cx);
    let rows: Vec<&str> = page.rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(
        rows,
        [
            "Continue",
            "In Progress",
            "Authors",
            "All Books",
            "Finished"
        ]
    );
    assert_eq!(
        page.values[1].as_deref(),
        Some("1"),
        "the finished one not in it"
    );
    assert_eq!(page.values[4].as_deref(), Some("1"));
}

#[test]
fn a_book_s_chapters_list_from_the_books_across_its_parts_on_the_one_it_was_left_in() {
    use crate::books::{Place, Shelf};
    use crate::music::ReadAhead;
    let (_d, mut lib) = library(&[
        "../Audiobooks/Wilde - Tales/01 one.m4b",
        "../Audiobooks/Wilde - Tales/02 two.mp3",
    ]);
    let book = BookId(0);
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    (lib.tracks[parts[0]].secs, lib.tracks[parts[1]].secs) = (180.0, 90.0);
    // The first part's own chapters, read; the second has none, so is one.
    let mut chapters = ReadAhead::default();
    chapters.want(&parts);
    let chapter = |start, title: &str| Chapter {
        start,
        title: title.into(),
    };
    chapters.put(parts[0], vec![chapter(0.0, "One"), chapter(60.0, "Two")]);
    chapters.put(parts[1], Vec::new());
    let mut shelf = Shelf::new();
    let left = Place {
        part: 0,
        pos: 70.0,
        when: 1,
        ..Place::default()
    };
    shelf.set_place(&lib, book, left);
    let cx = Context {
        shelf: &shelf,
        chapters: &chapters,
        ..Context::new(&lib, at(0))
    };
    let mut b = Browser::default();
    b.push(View::BookList(crate::BookList::All), 0);
    // X on a book, as Y lists its bookmarks.
    assert_eq!(b.press(&cx, Key::X), Outcome::None);
    assert_eq!(b.view(), View::BookChapters(book));
    let page = b.page(&cx);
    let titles: Vec<_> = page.rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(titles, ["One", "Two", "two"]);
    assert_eq!(page.selected, 1, "on the one it was left in");
    assert!(page.rows[0].faint && page.rows[1].playing);
    let lengths: Vec<_> = page.values.iter().map(|v| v.as_deref()).collect();
    assert_eq!(lengths, [Some("1:00"), Some("2:00"), Some("1:30")]);
    b.press(&cx, Key::Down);
    let picked = b.press(&cx, Key::A);
    let second = Outcome::BookChapter {
        book,
        part: 1,
        chapter: None,
    };
    assert_eq!(picked, second, "the part with none, from its start");
    assert_eq!(b.view(), View::NowPlaying, "on to play it");
    // The book's menu lists them too, first.
    b.press(&cx, Key::B);
    b.press(&cx, Key::Start);
    assert_eq!(b.view(), View::BookMenu(book));
    assert_eq!(b.press(&cx, Key::A), Outcome::None);
    assert_eq!(b.view(), View::BookChapters(book));
}

#[test]
fn a_book_s_chapters_are_numbered_through_the_book_as_an_album_s_songs_are() {
    let (_d, lib) = library(&[
        "../Audiobooks/Wilde - Tales/01 one.mp3",
        "../Audiobooks/Wilde - Tales/02 two.mp3",
        "../Audiobooks/Wilde - Tales/03 three.mp3",
    ]);
    let mut b = Browser::default();
    let cx = Context::new(&lib, at(0));
    b.push(View::BookChapters(BookId(0)), 0);
    let page = b.page(&cx);
    let numbers: Vec<&str> = page.rows.iter().map(|r| r.before.as_str()).collect();
    assert_eq!(numbers, ["01", "02", "03"]);
}
