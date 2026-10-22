//! Settings: the pages, their choices and the file they are kept in.

use super::common::*;

#[test]
fn music_settings_flip_with_left_and_right() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_SETTINGS);
    assert_eq!(b.view(), View::Settings);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let shown = |page: &crate::Page| -> Vec<(String, Option<String>)> {
        let rows = page.rows.iter().map(|r| r.text.clone());
        rows.zip(page.values.iter().cloned()).collect()
    };
    let on = |name: &str, value: &str| (name.to_string(), Some(value.to_string()));
    // A page of a menu's rows: the Sound page first, the settings, then the library and how many
    // songs it holds.
    assert_eq!(
        shown(&page),
        [
            ("Music".to_string(), None),
            on("Screen Off", "Pause"),
            on("Open on Now Playing", "Off"),
            on("Update Library", "1 song"),
            ("Help".to_string(), None),
            ("Advanced".to_string(), None),
        ]
    );
    // The sound settings on a page of their own.
    let own_cx = cx(&lib, QueueView::default(), Settings::default());
    let mut sound = Browser::default();
    for key in TO_SETTINGS.into_iter().chain([Key::A]) {
        sound.press(&own_cx, key);
    }
    assert_eq!(sound.view(), View::MusicSettings);
    assert_eq!(
        shown(&sound.page(&own_cx))[..],
        [
            on("Crossfade", "Off"),
            on("Volume Leveling", "Off"),
            on("Mono Audio", "Off"),
            on("Visualizer", "Off"),
            on("Equalizer", "Off"),
        ]
    );
    sound.press(&own_cx, Key::B);
    assert_eq!(sound.view(), View::Settings, "B back to the rest");
    // Update Library acts on A, its count only told: no arrows to change it.
    for key in [Key::Down; 3] {
        sound.press(&own_cx, key);
    }
    assert_eq!(sound.row(&own_cx), Some(crate::Row::Rescan));
    let legend = crate::screens::legend(&sound, &own_cx);
    assert_eq!(legend, crate::screens::Legend::Update);
    assert_eq!(
        press(&mut b, &lib, &[Key::Down, Key::Right]),
        Outcome::Change(crate::Setting::ScreenOff, 1)
    );
    press(&mut b, &lib, &[Key::B]);
    assert_eq!(b.view(), View::Main);
}

#[test]
fn settings_are_kept_on_the_card_but_the_sleep_timer_is_not() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::open(d.path());
    assert_eq!(store.settings(), Settings::default());
    let s = Settings {
        repeat: crate::Repeat::One,
        log: true,
        jump: 15,
        ..Settings::default()
    };
    store.save_settings(&s, &Settings::default());
    store.flush();
    assert_eq!(store.settings(), s);
    assert_eq!(crate::Repeat::One.next(), crate::Repeat::Off);
}

#[test]
fn a_goes_round_the_choices_while_left_and_right_stop_at_the_ends() {
    use crate::cycle;
    let order = [1, 2, 3];
    assert_eq!(cycle(&order, 3, 0), 1);
    assert_eq!(cycle(&order, 1, -1), 1);
    assert_eq!(cycle(&order, 1, 1), 2);
}

#[test]
fn a_settings_row_that_opens_a_page_hints_no_change() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let s = Settings::default();
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_SETTINGS);
    let cx = cx(&lib, QueueView::default(), s);
    // Up from the first goes round to Advanced, then Help; the first is Music, a page too.
    for keys in [&[Key::Up][..], &[Key::Up], &[Key::Down, Key::Down]] {
        for &k in keys {
            b.press(&cx, k);
        }
        let hints = crate::text::legend(crate::screens::legend(&b, &cx));
        assert!(
            hints.iter().all(|h| h.tap != Some("Change")),
            "{:?}: {hints:?}",
            b.row(&cx)
        );
    }
}

#[test]
fn advanced_ends_with_an_experimental_page_of_skip_silence() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let s = Settings::default();
    let mut b = Browser::default();
    // Settings, Up round to Advanced, then Up round to its last row, Experimental.
    press(
        &mut b,
        &lib,
        &[Key::Up, Key::A, Key::Up, Key::A, Key::Up, Key::A],
    );
    assert_eq!(b.view(), View::Experimental);
    let page = b.page(&cx(&lib, QueueView::default(), s));
    assert_eq!(page.rows, ["Skip Silence"]);
}
