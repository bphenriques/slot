//! The queue as it plays: skips, repeats, shuffles, the saved queue and gapless.

use super::common::*;

#[test]
fn the_player_moves_on_at_the_end_of_its_own_track_only() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    assert_eq!(
        played(p.play(&lib, Queue::new(ids(&[0, 1]), 0, None))),
        lib.tracks[0].path
    );
    assert!(p.owns_audio());
    let stale = lib.tracks[1].path.clone();
    assert_eq!(p.on_event(&lib, Event::End(stale)), None);
    let next = p.on_event(&lib, Event::End(lib.tracks[0].path.clone()));
    assert_eq!(played(next), lib.tracks[1].path);
    assert_eq!(
        p.on_event(&lib, Event::End(lib.tracks[1].path.clone())),
        None
    );
    assert_eq!(p.state(), PlayState::Stopped);
    assert!(!p.owns_audio());
}

#[test]
fn a_track_that_cannot_play_is_skipped_but_a_run_of_them_stops_the_queue() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new((0..5).map(TrackId).collect(), 0, None));
    let error = || Event::Error("cannot open".into());
    assert_eq!(played(p.on_event(&lib, error())), lib.tracks[1].path);
    heard(&mut p, 1.0, 9.0);
    assert_eq!(played(p.on_event(&lib, error())), lib.tracks[2].path);
    assert_eq!(played(p.on_event(&lib, error())), lib.tracks[3].path);
    assert_eq!(p.on_event(&lib, error()), None);
    assert_eq!(p.state(), PlayState::Stopped);
}

#[test]
fn pause_resume_and_stop_hand_the_sound_back() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0]), 0, None));
    assert_eq!(p.toggle(&lib), Some(Command::Pause));
    assert!(!p.owns_audio());
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
    assert!(p.owns_audio());
    assert_eq!(p.stop(), Some(Command::Stop));
    assert!(p.current().is_none() && !p.owns_audio());
    assert_eq!(p.pause(), None);
}

#[test]
fn shuffle_keeps_every_track_once() {
    let (_d, lib) = library(&["X/A/1 a.mp3", "X/A/2 b.mp3", "X/A/3 c.mp3", "X/A/4 d.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2, 3]), 0, Some(42)));
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(p.current().unwrap());
        p.skip(&lib, 1);
    }
    seen.sort();
    assert_eq!(seen, ids(&[0, 1, 2, 3]));
}

#[test]
fn the_music_stays_audible_until_the_player_confirms_it_stopped() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0]), 0, None));
    reported(&mut p, PlayState::Playing);
    p.stop();
    assert!(!p.owns_audio(), "a game may start opening the codec");
    assert!(p.audible(), "but the player has not said so yet");
    said(&mut p, PlayState::Stopped, true);
    assert!(p.audible(), "stopped, but its last moment still fading out");
    said(&mut p, PlayState::Stopped, false);
    assert!(!p.audible(), "quiet: the codec is slot's again");
    // Paused alike: the codec keeps the music's level through the fade, so it is not heard louder.
    p.play(&lib, Queue::new(ids(&[0]), 0, None));
    reported(&mut p, PlayState::Playing);
    p.pause();
    assert!(!p.owns_audio());
    said(&mut p, PlayState::Paused, true);
    assert!(p.audible());
    said(&mut p, PlayState::Paused, false);
    assert!(!p.audible());
}

#[test]
fn pausing_for_a_game_keeps_the_queue_for_later() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 1, None));
    p.edit().enqueue(&ids(&[0]));
    assert_eq!(p.pause(), Some(Command::Pause));
    assert_eq!((p.current(), p.place()), (Some(TrackId(1)), (2, 3)));
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
}

#[test]
fn repeat_one_replays_and_repeat_all_wraps_round() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 1, None));
    p.set_repeat(crate::Repeat::One);
    let again = p.on_event(&lib, Event::End(lib.tracks[1].path.clone()));
    assert_eq!(played(again), lib.tracks[1].path);
    p.set_repeat(crate::Repeat::All);
    let wrapped = p.on_event(&lib, Event::End(lib.tracks[1].path.clone()));
    assert_eq!(played(wrapped), lib.tracks[0].path);
    assert_eq!(played(p.skip(&lib, -1)), lib.tracks[1].path);
}

#[test]
fn the_queue_comes_back_from_the_card_paused_where_it_was() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    let saved = SavedQueue {
        tracks: ids(&[2, 0, 1]),
        at: 1,
        pos: 42.5,
        original: None,
    };
    let text = saved.encode(card);
    assert!(text.lines().any(|l| l == "Music/X/A/01 one.mp3"));
    assert_eq!(SavedQueue::decode(&text, card), saved);
    let mut p = Player::default();
    p.restore(Queue::new(ids(&[2, 0, 1]), 1, None), 42.5, false);
    assert!(p.resumable() && !p.owns_audio());
    assert_eq!(
        p.toggle(&lib),
        Some(Command::Play {
            path: lib.tracks[0].path.clone(),
            at: 42.5,
            speed: 1.0
        })
    );
}

#[test]
fn a_queued_track_gone_from_the_library_is_dropped_and_its_place_moves_on() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/03 three.mp3"]);
    let text = "#EXTM3U\n#SLOT-AT:1\n#SLOT-POS:30.0\nMusic/X/A/01 one.mp3\nMusic/X/A/02 gone.mp3\nMusic/X/A/03 three.mp3\n";
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    let SavedQueue {
        tracks, at, pos, ..
    } = SavedQueue::decode(text, card);
    assert_eq!((tracks, at, pos), (ids(&[0, 1]), 1, 0.0));
}

#[test]
fn repeat_all_goes_round_the_same_shuffle() {
    let files: Vec<String> = (1..=8).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.set_repeat(crate::Repeat::All);
    p.play(&lib, Queue::new((0..8).map(TrackId).collect(), 0, Some(7)));
    let first: Vec<TrackId> = p.queue().tracks.to_vec();
    assert_eq!(p.up_next(), Some(first[1]));
    p.skip(&lib, 7);
    assert_eq!(p.up_next(), Some(first[0]), "round to the start");
    p.skip(&lib, 1);
    assert_eq!(p.queue().tracks, first.as_slice());
    assert_eq!(p.current(), Some(first[0]));
    p.edit().toggle_shuffle(1);
    assert_eq!(
        p.queue().tracks,
        (0..8).map(TrackId).collect::<Vec<_>>(),
        "back in order"
    );
}

#[test]
fn the_player_says_when_the_queue_is_worth_keeping() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    assert!(!p.take_changed());
    p.play(&lib, Queue::new(ids(&[0]), 0, None));
    assert!(p.take_changed() && !p.take_changed(), "once per change");
    heard(&mut p, 3.0, 9.0);
    assert!(!p.take_changed(), "the clock ticking is not one");
    p.pause();
    assert!(p.take_changed());
}

#[test]
fn shuffling_the_queue_keeps_the_track_playing_and_undoes_to_the_old_order() {
    let files: Vec<String> = (1..=6).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new((0..6).map(TrackId).collect(), 3, None));
    p.edit().toggle_shuffle(9);
    assert!(p.shuffled());
    assert_eq!(p.current(), Some(TrackId(3)));
    assert_eq!(p.queue().tracks[0], TrackId(3), "what plays comes first");
    p.edit().toggle_shuffle(9);
    assert!(!p.shuffled());
    assert_eq!(p.queue().tracks, ids(&[0, 1, 2, 3, 4, 5]));
    assert_eq!(p.current(), Some(TrackId(3)));
}

#[test]
fn play_next_goes_right_after_what_plays_and_survives_unshuffling() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2]), 0, None));
    p.edit().play_next(&ids(&[4]));
    assert_eq!(p.queue().tracks, ids(&[0, 4, 1, 2]));
    p.edit().toggle_shuffle(3);
    p.edit().play_next(&ids(&[3]));
    p.edit().toggle_shuffle(3);
    assert_eq!(p.queue().tracks, ids(&[0, 3, 4, 1, 2]));
}

#[test]
fn what_is_added_plays_after_what_was_added_before_ahead_of_the_rest() {
    let files: Vec<String> = (1..=7).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2]), 0, None));
    p.edit().enqueue(&ids(&[3]));
    p.edit().enqueue(&ids(&[4]));
    assert_eq!(
        p.queue().tracks,
        ids(&[0, 3, 4, 1, 2]),
        "in the order added"
    );
    p.edit().play_next(&ids(&[5]));
    assert_eq!(
        p.queue().tracks,
        ids(&[0, 5, 3, 4, 1, 2]),
        "play next goes first"
    );
    p.jump(&lib, 1);
    p.edit().remove(2);
    p.edit().enqueue(&ids(&[6]));
    assert_eq!(
        p.queue().tracks,
        ids(&[0, 5, 4, 6, 1, 2]),
        "still after the added"
    );
    p.jump(&lib, 4);
    p.edit().enqueue(&ids(&[3]));
    assert_eq!(
        p.queue().tracks,
        ids(&[0, 5, 4, 6, 1, 3, 2]),
        "once played, after what plays"
    );
}

#[test]
fn the_queue_can_lose_and_move_tracks_around_what_plays() {
    let files: Vec<String> = (1..=5).map(|i| format!("X/A/{i} t{i}.mp3")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_d, lib) = library(&files);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2, 3, 4]), 2, None));
    p.edit().remove(2);
    assert_eq!(
        p.queue().tracks,
        ids(&[0, 1, 2, 3, 4]),
        "not the one playing"
    );
    p.edit().remove(0);
    assert_eq!(
        (p.queue().tracks, p.current()),
        (&ids(&[1, 2, 3, 4])[..], Some(TrackId(2)))
    );
    p.edit().move_to(3, 2);
    assert_eq!(p.queue().tracks, ids(&[1, 2, 4, 3]));
    assert_eq!(p.current(), Some(TrackId(2)));
}

#[test]
fn moving_a_track_keeps_the_playing_one_playing() {
    let mut q = Queue::new(ids(&[10, 11, 12, 13]), 1, None);
    q.move_to(3, 0);
    assert_eq!(
        (q.tracks(), q.current()),
        (&ids(&[13, 10, 11, 12])[..], Some(TrackId(11)))
    );
    q.move_to(2, 3);
    assert_eq!(
        (q.tracks(), q.current()),
        (&ids(&[13, 10, 12, 11])[..], Some(TrackId(11)))
    );
}

#[test]
fn up_next_is_the_next_in_the_queue_or_the_first_again_under_repeat_all() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 0, None));
    assert_eq!(p.up_next(), Some(TrackId(1)));
    p.skip(&lib, 1);
    assert_eq!(p.up_next(), None, "the end");
    p.set_repeat(crate::Repeat::All);
    assert_eq!(p.up_next(), Some(TrackId(0)));
}

#[test]
fn the_queue_opens_at_what_plays_in_a_shuffled_queue_after_skips() {
    let files: Vec<String> = (1..=60).map(|i| format!("X/A/{i:02} t{i}.mp3")).collect();
    let (_d, lib) = library(&files.iter().map(String::as_str).collect::<Vec<_>>());
    let mut p = Player::default();
    p.set_repeat(crate::Repeat::One);
    p.play(&lib, Queue::new((0..60).map(TrackId).collect(), 0, Some(7)));
    for _ in 0..36 {
        p.skip(&lib, 1);
    }
    let mut b = Browser::default();
    let s = Settings::default();
    b.press(&cx(&lib, p.queue(), s), Key::Select);
    b.press(&cx(&lib, p.queue(), s), Key::Select);
    assert_eq!(b.view(), View::Queue);
    let page = b.page(&cx(&lib, p.queue(), s));
    assert_eq!(
        page.rows.len(),
        crate::browse::RICH_ROWS + 1,
        "no Play All: a row more {:?}",
        page.rows
    );
    assert!(
        page.rows[page.selected].playing,
        "at what plays, the 36 played above"
    );
    assert!(page.rows.iter().all(|r| !r.text.is_empty()));
}

#[test]
fn back_starts_the_track_over_once_it_has_played_three_seconds() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 1, None));
    heard(&mut p, 10.0, 60.0);
    assert_eq!(p.skip(&lib, -1), Some(Command::Seek(0.0)), "the start");
    assert_eq!(p.current(), Some(TrackId(1)));
    heard(&mut p, 2.5, 60.0);
    assert_eq!(
        played(p.skip(&lib, -1)),
        lib.tracks[0].path,
        "the one before"
    );
}

#[test]
fn a_saved_queue_reads_resume_before_the_library_is_read() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.wait_for_saved();
    let b = Browser::default();
    let page = b.page(&cx(&lib, p.queue(), Settings::default()));
    assert_eq!(page.rows[0], "Resume", "not Now Playing, then Resume");
    p.restore(Queue::new(ids(&[0]), 0, None), 0.0, false);
    let page = b.page(&cx(&lib, p.queue(), Settings::default()));
    assert_eq!(page.rows[0], "Resume");
}

#[test]
fn resume_before_the_saved_queue_is_back_plays_once_it_is() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    p.wait_for_saved();
    let mut b = Browser::default();
    assert_eq!(
        b.press(&cx(&lib, p.queue(), Settings::default()), Key::A),
        Outcome::Toggle,
        "Resume plays, not only opens"
    );
    assert_eq!(b.view(), View::NowPlaying);
}

#[test]
fn gapless_tells_the_player_what_follows_and_the_queue_follows_it() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let path = |t: usize| lib.tracks[t].path.clone();
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2]), 0, None));
    assert_eq!(p.upcoming(&lib, false), None, "not asked");
    assert_eq!(p.upcoming(&lib, true), Some(Command::Next(Some(path(1)))));
    assert_eq!(p.upcoming(&lib, true), None, "told once");
    // It went on into two: the queue follows, with nothing to play.
    assert_eq!(p.on_event(&lib, Event::Into(path(1))), None);
    assert_eq!(p.current(), Some(TrackId(1)));
    assert_eq!(p.upcoming(&lib, true), Some(Command::Next(Some(path(2)))));
    // Into what slot no longer meant: it plays what it does.
    p.edit().remove(2);
    assert_eq!(
        p.upcoming(&lib, true),
        Some(Command::Next(None)),
        "nothing after the last"
    );
    assert_eq!(
        p.on_event(&lib, Event::Into(path(2))),
        Some(Command::Stop),
        "nothing it means to play: the player stops"
    );
    // Repeat One goes on into the same song.
    let mut p = Player::default();
    p.set_repeat(crate::Repeat::One);
    p.play(&lib, Queue::new(ids(&[0, 1]), 0, None));
    assert_eq!(p.upcoming(&lib, true), Some(Command::Next(Some(path(0)))));
}

#[test]
fn gapless_never_takes_a_book_past_its_end() {
    let (_d, lib) = books();
    let parts = lib.albums[lib.books[0].album].tracks.clone();
    let mut p = Player::default();
    p.set_repeat(crate::Repeat::All);
    p.play(&lib, Queue::new(parts.clone(), parts.len() - 1, None));
    assert_eq!(
        p.upcoming(&lib, true),
        None,
        "the last part ends, to finish the book"
    );
}

#[test]
fn a_song_skipped_to_while_paused_waits_to_be_played() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 0, None));
    p.pause();
    let cued = p.skip(&lib, 1);
    assert!(
        matches!(&cued, Some(Command::Cue { path, at, .. }) if *path == lib.tracks[1].path && *at == 0.0),
        "{cued:?}"
    );
    assert_eq!(p.state(), PlayState::Paused);
    assert_eq!(p.current(), Some(TrackId(1)));
}

#[test]
fn a_place_asked_for_is_shown_until_the_player_is_there() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1]), 0, None));
    heard(&mut p, 150.0, 200.0);
    assert_eq!(p.time(), (150.0, 200.0), "as the player has it");
    p.skip(&lib, 1);
    heard_at(&mut p, 150.5, 200.0, 1000);
    assert_eq!(
        p.time(),
        (0.0, 0.0),
        "the next song's start, not the last's place"
    );
    heard_at(&mut p, 150.6, 200.0, 1499);
    assert_eq!(p.time(), (0.0, 0.0), "for half a second at most");
    heard_at(&mut p, 150.7, 200.0, 1500);
    assert_eq!(
        p.time(),
        (150.7, 200.0),
        "then the player's own, whatever it says"
    );
    heard(&mut p, 0.1, 180.0);
    assert_eq!(
        p.time(),
        (0.1, 180.0),
        "the next file open: the player's own"
    );
    p.seek(30);
    assert_eq!(p.time().0, 30.1, "a seek shows at once");
}

#[test]
fn a_shuffled_queue_comes_back_from_the_card_shuffled_with_its_order_to_go_back_to() {
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    let mut q = Queue::new(ids(&[0, 1, 2]), 0, None);
    q.go(1);
    q.toggle_shuffle(3);
    let text = SavedQueue::of(&q, 12.0).encode(card);
    let mut back = SavedQueue::decode(&text, card).queue();
    assert!(back.shuffled());
    assert_eq!((back.tracks(), back.at()), (q.tracks(), q.at()));
    back.toggle_shuffle(0);
    assert_eq!((back.tracks(), back.at()), (ids(&[0, 1, 2]).as_slice(), 1));
    // A song gone from the card leaves the rest of the order as it was.
    let (d, lib) = library(&["X/A/01 one.mp3", "X/A/03 three.mp3"]);
    let card = Card {
        root: d.path(),
        lib: &lib,
    };
    let text = "#EXTM3U\n#SLOT-AT:0\n#SLOT-POS:0\n#SLOT-ORDER:2,0,1\nMusic/X/A/02 two.mp3\nMusic/X/A/03 three.mp3\nMusic/X/A/01 one.mp3\n";
    let mut back = SavedQueue::decode(text, card).queue();
    assert_eq!(back.tracks(), ids(&[1, 0]));
    back.toggle_shuffle(0);
    assert_eq!(back.tracks(), ids(&[0, 1]), "one, then three");
    // An order that is not each place once is not trusted.
    let text = "#EXTM3U\n#SLOT-ORDER:0,0\nMusic/X/A/03 three.mp3\nMusic/X/A/01 one.mp3\n";
    assert!(!SavedQueue::decode(text, card).queue().shuffled());
}

#[test]
fn a_seek_before_the_file_is_open_moves_from_the_place_asked_not_back_to_the_start() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut p = Player::default();
    // A book opened half an hour in, the player not yet saying its length.
    p.play_at(&lib, Queue::new(ids(&[0]), 0, None), 1800.0);
    assert_eq!(p.seek(30), Some(Command::Seek(1830.0)));
    assert_eq!(
        p.seek(-3600),
        Some(Command::Seek(0.0)),
        "never before the start"
    );
    // Once known, never past the end.
    heard_at(&mut p, 1830.0, 1900.0, 2000);
    assert_eq!(p.seek(300), Some(Command::Seek(1899.0)));
}

#[test]
fn a_song_that_will_not_open_while_paused_is_passed_over_and_the_next_waits() {
    let (_d, lib) = library(&["X/A/01 one.mp3", "X/A/02 two.mp3", "X/A/03 three.mp3"]);
    let mut p = Player::default();
    p.play(&lib, Queue::new(ids(&[0, 1, 2]), 0, None));
    heard(&mut p, 10.0, 100.0);
    p.pause();
    assert!(matches!(p.skip(&lib, 1), Some(Command::Cue { .. })));
    let cued = p.on_event(&lib, Event::Error("cannot open".into()));
    assert!(
        matches!(&cued, Some(Command::Cue { path, .. }) if *path == lib.tracks[2].path),
        "{cued:?}"
    );
    assert_eq!(p.state(), PlayState::Paused);
    assert_eq!(p.toggle(&lib), Some(Command::Resume));
    // The last will not open either: nothing is left to wait on.
    p.pause();
    assert_eq!(p.on_event(&lib, Event::Error("cannot open".into())), None);
    assert_eq!(p.state(), PlayState::Stopped);
}
