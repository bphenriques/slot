//! Music Settings: the pages, their choices and the file they are kept in.

use super::common::*;

#[test]
fn music_settings_flip_with_left_and_right() {
    let (_d, lib) = library(&["X/A/01 one.mp3"]);
    let mut b = Browser::default();
    press(&mut b, &lib, &TO_SETTINGS);
    assert_eq!(b.view(), View::Settings);
    let page = b.page(&cx(&lib, QueueView::default(), Settings::default()));
    let shown = |page: &slot_music::Page| -> Vec<(String, Option<String>)> {
        let rows = page.rows.iter().map(|r| r.text.clone());
        rows.zip(page.values.iter().cloned()).collect()
    };
    let on = |name: &str, value: &str| (name.to_string(), Some(value.to_string()));
    // A page of a menu's rows: listening first, then looks.
    assert_eq!(
        shown(&page),
        [
            on("Screen Off", "Pause"),
            on("Pocket Lock", "On"),
            on("Pocket Vibration", "On"),
            ("Sound".to_string(), None),
            on("List Size", "Normal"),
            on("Dynamic Color", "On"),
            on("Theme", "Slot"),
        ]
    );
    // The sound settings on a page of their own.
    let own_cx = cx(&lib, QueueView::default(), Settings::default());
    let page = b.page(&own_cx);
    assert_eq!(shown(&page)[3], ("Sound".to_string(), None));
    let mut sound = Browser::default();
    for key in TO_SETTINGS
        .into_iter()
        .chain([Key::Down; 3])
        .chain([Key::A])
    {
        sound.press(&own_cx, key);
    }
    assert_eq!(sound.view(), View::Sound);
    assert_eq!(
        shown(&sound.page(&own_cx))[..],
        [
            on("Crossfade", "Off"),
            on("Volume Leveling", "Off"),
            on("Mono Audio", "Off"),
        ]
    );
    sound.press(&own_cx, Key::B);
    assert_eq!(sound.view(), View::Settings, "B back to the rest");
    assert_eq!(
        press(&mut b, &lib, &[Key::Right]),
        Outcome::Change(slot_music::Setting::ScreenOff, 1)
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
        repeat: slot_music::Repeat::One,
        clock: true,
        up_next: false,
        color: false,
        jump: 15,
        ..Settings::default()
    };
    store.save_settings(&s, &Settings::default());
    store.flush();
    assert_eq!(store.settings(), s);
    assert_eq!(slot_music::Repeat::One.next(), slot_music::Repeat::Off);
}

#[test]
fn a_goes_round_the_choices_while_left_and_right_stop_at_the_ends() {
    use slot_music::cycle;
    let order = [1, 2, 3];
    assert_eq!(cycle(&order, 3, 0), 1);
    assert_eq!(cycle(&order, 1, -1), 1);
    assert_eq!(cycle(&order, 1, 1), 2);
}
