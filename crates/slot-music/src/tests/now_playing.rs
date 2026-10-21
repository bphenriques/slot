//! Now Playing and the lyrics.

use super::common::*;

#[test]
fn now_playing_skips_with_l1_r1_and_seeks_with_l2_r2() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Select]);
    assert_eq!(b.view(), View::NowPlaying);
    let cx = Context::new(&lib, at(0));
    assert_eq!(b.mode(&cx, Key::X), Mode::Once);
    assert_eq!(
        press(&mut b, &lib, &[Key::Up, Key::Down]),
        Outcome::None,
        "repeat and shuffle are in START's menu"
    );
    assert_eq!(press(&mut b, &lib, &[Key::L1]), Outcome::Skip(-1));
    assert_eq!(press(&mut b, &lib, &[Key::R1]), Outcome::Skip(1));
    assert_eq!(press(&mut b, &lib, &[Key::L2]), Outcome::Seek(-10));
    assert_eq!(press(&mut b, &lib, &[Key::R2]), Outcome::Seek(10));
    assert_eq!(
        press(&mut b, &lib, &[Key::Right]),
        Outcome::Skip(1),
        "as R1 does"
    );
}

#[test]
fn on_now_playing_every_back_and_on_key_skips_tapped_and_seeks_held() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Select]);
    let cx = Context::new(&lib, at(0));
    for (back, on) in [
        (Key::Left, Key::Right),
        (Key::L1, Key::R1),
        (Key::L2, Key::R2),
    ] {
        assert_eq!(
            b.mode(&cx, back),
            Mode::HoldRepeat,
            "{back:?} waits, so a combo can start"
        );
        assert!(matches!(b.held(&cx, back), Outcome::Seek(s) if s < 0));
        assert!(matches!(b.held(&cx, on), Outcome::Seek(s) if s > 0));
    }
}

#[test]
fn x_opens_the_lyrics_and_closes_them_and_up_and_down_scroll_from_the_line_sung() {
    use crate::LyricsAt;
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &[Key::Select]);
    let cx = Context::new(&lib, at(0));
    b.press(&cx, Key::X);
    assert_eq!(b.view(), View::NowPlaying, "no lyrics, nothing to open");
    let sung = Context {
        lyrics: LyricsAt {
            lines: 10,
            sung: Some(4),
            timed: false,
        },
        ..Context::new(&lib, at(0))
    };
    b.press(&sung, Key::X);
    assert_eq!(b.view(), View::Lyrics);
    assert_eq!(b.mode(&sung, Key::Down), Mode::Repeat);
    b.press(&sung, Key::Down);
    assert_eq!(b.at(), 5, "on from the line sung");
    for _ in 0..9 {
        b.press(&sung, Key::Down);
    }
    assert_eq!(b.at(), 9, "not past the last");
    assert_eq!(
        b.press(&sung, Key::A),
        Outcome::Toggle,
        "the rest as on Now Playing"
    );
    let timed = Context {
        lyrics: LyricsAt {
            timed: true,
            ..sung.lyrics
        },
        ..Context::new(&lib, at(0))
    };
    b.press(&timed, Key::Up);
    assert_eq!(
        b.press(&timed, Key::A),
        Outcome::FromLine(8),
        "timed, from the line"
    );
    assert_eq!(
        b.press(&timed, Key::A),
        Outcome::Toggle,
        "then following again"
    );
    // Scrolled by hand, they stay four seconds where they were left, then follow the song.
    let at_ms = |ms| Context {
        now: at(ms),
        ..Context::new(&lib, at(0))
    };
    let timed_at = |ms| Context {
        lyrics: timed.lyrics,
        ..at_ms(ms)
    };
    b.press(&timed_at(10_000), Key::Up);
    assert!(
        matches!(b.press(&timed_at(13_999), Key::A), Outcome::FromLine(_)),
        "still where left"
    );
    b.press(&timed_at(20_000), Key::Up);
    assert_eq!(
        b.press(&timed_at(24_000), Key::A),
        Outcome::Toggle,
        "following again"
    );
    b.press(&sung, Key::X);
    assert_eq!(b.view(), View::NowPlaying, "X closes them");
}
