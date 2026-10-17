//! slot's player, heard through an output that keeps what it is given: WAV files made here,
//! played, paused, moved through and stopped.

use std::path::{Path, PathBuf};
use std::time::Duration;

use slot_player::{Capture, Command, Event, Handle, PlayState};

const RATE: f64 = 48_000.0;
const WAIT: Duration = Duration::from_secs(5);

/// A WAV of `secs` seconds, 16-bit at `rate`, every sample a quarter of full scale.
fn wav(dir: &Path, name: &str, secs: f64, rate: u32, channels: u16) -> PathBuf {
    wav_of(dir, name, secs, rate, channels, |_, _| 8192)
}

/// A WAV of `secs` seconds, 16-bit at `rate`, each sample `sample(frame, channel)`.
fn wav_of(
    dir: &Path,
    name: &str,
    secs: f64,
    rate: u32,
    channels: u16,
    sample: impl Fn(u32, u16) -> i16,
) -> PathBuf {
    let frames = (secs * f64::from(rate)) as u32;
    let data = frames * u32::from(channels) * 2;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(channels.to_le_bytes());
    b.extend(rate.to_le_bytes());
    b.extend((rate * u32::from(channels) * 2).to_le_bytes());
    b.extend((channels * 2).to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    for frame in 0..frames {
        for channel in 0..channels {
            b.extend(sample(frame, channel).to_le_bytes());
        }
    }
    let path = dir.join(name);
    std::fs::write(&path, b).unwrap();
    path
}

/// A player into a fresh capture, played `pace` times as fast as heard, or as fast as it goes.
fn player(pace: Option<f64>) -> (Handle, Capture) {
    let capture = Capture {
        pace,
        ..Capture::default()
    };
    let handle = Handle::on(capture.clone());
    assert_eq!(handle.wait(WAIT), Some(Event::Connected(true)));
    (handle, capture)
}

fn play(path: &Path, at: f64) -> Command {
    Command::Play {
        path: path.into(),
        at,
        speed: 1.0,
    }
}

/// The events up to and with the first that `last` picks, failing if none comes in time.
fn until(handle: &Handle, last: impl Fn(&Event) -> bool) -> Vec<Event> {
    let mut told = Vec::new();
    loop {
        let event = handle.wait(WAIT).expect("an event in time");
        let done = last(&event);
        told.push(event);
        if done {
            return told;
        }
    }
}

/// The seconds of sound the capture holds.
fn heard(capture: &Capture) -> f64 {
    capture.samples.lock().unwrap().len() as f64 / 2.0 / RATE
}

fn near(a: f64, b: f64) -> bool {
    // A chunk of slack: the last is filled out with silence.
    (a - b).abs() < 0.1
}

#[test]
fn a_file_plays_to_its_end_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    // Mono at 44.1 kHz: brought to the device's stereo at 48.
    let path = wav(dir.path(), "a.wav", 0.5, 44_100, 1);
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(told[0], Event::State(PlayState::Playing));
    assert!(
        matches!(told[1], Event::Pos { at, len } if at == 0.0 && near(len, 0.5)),
        "{told:?}"
    );
    let n = told.len();
    assert_eq!(
        told[n - 2..],
        [Event::State(PlayState::Stopped), Event::End(path)]
    );
    assert!(near(heard(&capture), 0.5), "{}", heard(&capture));
    assert!(capture.samples.lock().unwrap().iter().any(|&s| s > 0.1));
}

#[test]
fn a_file_starts_where_it_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 1.0, 48_000, 2);
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.75));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!(near(heard(&capture), 0.25), "{}", heard(&capture));
}

#[test]
fn a_pause_keeps_the_moment_heard_and_playing_on_goes_on_from_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, capture) = player(Some(4.0));
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(150));
    handle.send(Command::Pause);
    until(&handle, |e| *e == Event::State(PlayState::Paused));
    let Some(Event::Pos { at, .. }) = handle.wait(WAIT) else {
        panic!("where it paused")
    };
    assert!(at > 0.2 && at < 1.5, "paused a way in: {at}");
    std::thread::sleep(Duration::from_millis(100));
    let paused = heard(&capture);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(heard(&capture), paused, "nothing written while paused");
    handle.send(Command::Resume);
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!(
        near(heard(&capture), 2.0),
        "all of it, once: {}",
        heard(&capture)
    );
}

#[test]
fn a_move_while_paused_plays_on_from_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, capture) = player(Some(4.0));
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(50));
    handle.send(Command::Pause);
    until(&handle, |e| *e == Event::State(PlayState::Paused));
    assert!(
        matches!(handle.wait(WAIT), Some(Event::Pos { .. })),
        "where it paused"
    );
    std::thread::sleep(Duration::from_millis(50));
    let before = heard(&capture);
    handle.send(Command::Seek(1.5));
    let told = until(&handle, |e| matches!(e, Event::Pos { .. }));
    assert!(
        matches!(told.last(), Some(&Event::Pos { at, .. }) if near(at, 1.5)),
        "{told:?}"
    );
    handle.send(Command::Resume);
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!(
        near(heard(&capture) - before, 0.5),
        "{}",
        heard(&capture) - before
    );
}

#[test]
fn a_skip_plays_the_next_at_once_and_only_its_end_is_told() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 0.25, 48_000, 2);
    let (handle, _capture) = player(Some(4.0));
    handle.send(play(&a, 0.0));
    std::thread::sleep(Duration::from_millis(50));
    handle.send(play(&b, 0.0));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(told.last(), Some(&Event::End(b)));
    assert!(!told.contains(&Event::End(a)));
}

#[test]
fn stop_ends_the_file_without_an_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, _capture) = player(Some(4.0));
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(50));
    handle.send(Command::Stop);
    until(&handle, |e| *e == Event::State(PlayState::Stopped));
    std::thread::sleep(Duration::from_millis(700));
    assert!(!handle.events().any(|e| matches!(e, Event::End(_))));
}

#[test]
fn a_file_that_will_not_play_is_an_error_and_nothing_more() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.mp3");
    std::fs::write(&broken, b"not a song").unwrap();
    let (handle, _capture) = player(None);
    for path in [broken, dir.path().join("missing.mp3")] {
        handle.send(play(&path, 0.0));
        assert!(matches!(handle.wait(WAIT), Some(Event::Error(_))));
    }
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(handle.events().count(), 0, "no state, no place");
}

/// An output whose device will not take sound.
struct Broken;

impl slot_player::Output for Broken {
    fn write(&mut self, _: &[f32]) -> Result<(), String> {
        Err("no sound".into())
    }

    fn delay(&mut self) -> u64 {
        0
    }

    fn flush(&mut self) {}

    fn release(&mut self) {}
}

#[test]
fn a_device_that_will_not_sound_is_told_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let handle = Handle::on(Broken);
    assert_eq!(handle.wait(WAIT), Some(Event::Connected(true)));
    handle.send(play(&path, 0.0));
    let told = until(&handle, |e| matches!(e, Event::Error(_)));
    assert_eq!(told.last(), Some(&Event::Error("no sound".into())));
}

/// A tone of `hz` at `rate`, a quarter of full scale.
fn tone(hz: f64, rate: u32) -> impl Fn(u32, u16) -> i16 {
    move |frame, _| {
        (8192.0 * (std::f64::consts::TAU * hz * f64::from(frame) / f64::from(rate)).sin()) as i16
    }
}

/// The pitch the capture holds, in Hz, from how often its left side rises through zero, over
/// the sound and not the silence it ends with.
fn pitch(capture: &Capture) -> f64 {
    let samples = capture.samples.lock().unwrap();
    let mut left: Vec<f32> = samples.iter().step_by(2).copied().collect();
    while left.last() == Some(&0.0) {
        left.pop();
    }
    let rises = |w: &&[f32]| w[0] < 0.0 && w[1] >= 0.0 && w[1] - w[0] > 0.01;
    let ups = left.windows(2).filter(rises).count();
    ups as f64 / (left.len() as f64 / RATE)
}

#[test]
fn sound_keeps_muse_s_headroom() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    let level = capture.samples.lock().unwrap()[1000];
    assert!((level - 0.25 * 0.794).abs() < 1e-3, "2 dB down: {level}");
}

#[test]
fn a_file_at_another_rate_keeps_its_pitch() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav_of(dir.path(), "a.wav", 1.0, 44_100, 1, tone(1000.0, 44_100));
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!(near(heard(&capture), 1.0), "{}", heard(&capture));
    assert!(
        (pitch(&capture) - 1000.0).abs() < 5.0,
        "{}",
        pitch(&capture)
    );
}

#[test]
fn mono_plays_both_sides_alike() {
    let dir = tempfile::tempdir().unwrap();
    // The left side only.
    let path = wav_of(dir.path(), "a.wav", 0.25, 48_000, 2, |_, c| {
        [8192, 0][usize::from(c)]
    });
    let (handle, capture) = player(None);
    handle.send(Command::Mono(true));
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    let frame = capture.samples.lock().unwrap()[1000..1002].to_vec();
    assert!(
        frame[0] == frame[1] && (frame[0] - 0.125 * 0.794).abs() < 1e-3,
        "{frame:?}"
    );
}

#[test]
fn a_book_at_twice_its_speed_takes_half_the_time_at_the_same_pitch() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav_of(dir.path(), "a.wav", 2.0, 48_000, 2, tone(440.0, 48_000));
    let (handle, capture) = player(None);
    handle.send(Command::Play {
        path: path.clone(),
        at: 0.0,
        speed: 2.0,
    });
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!((heard(&capture) - 1.0).abs() < 0.05, "{}", heard(&capture));
    assert!(
        (pitch(&capture) - 440.0).abs() < 10.0,
        "{}",
        pitch(&capture)
    );
}

#[test]
fn a_speed_set_while_playing_goes_on_from_the_moment_heard() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, _capture) = player(Some(4.0));
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(100));
    handle.send(Command::Speed(1.5));
    let told = until(&handle, |e| matches!(e, Event::Pos { at, .. } if *at > 0.0));
    let Some(&Event::Pos { at, .. }) = told.last() else {
        unreachable!()
    };
    assert!(at > 0.2 && at < 1.0, "on from where it was: {at}");
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    let last = told.iter().rev().find_map(|e| match e {
        Event::Pos { at, .. } => Some(*at),
        _ => None,
    });
    assert!(
        last.is_none_or(|at| at <= 2.0),
        "never past the end: {last:?}"
    );
}

#[test]
fn the_next_file_follows_with_no_gap_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 0.5, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 0.5, 48_000, 2);
    let (handle, capture) = player(Some(8.0));
    handle.send(play(&a, 0.0));
    handle.send(Command::Next(Some(b.clone())));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert!(told.contains(&Event::Into(b.clone())), "{told:?}");
    assert!(!told.contains(&Event::End(a)), "a went on into b");
    assert_eq!(told.last(), Some(&Event::End(b)));
    let samples = capture.samples.lock().unwrap();
    let sound = samples.iter().rposition(|&s| s != 0.0).unwrap() + 1;
    assert!(
        samples[..sound].iter().all(|&s| s != 0.0),
        "no silence between them"
    );
    assert!(
        near(sound as f64 / 2.0 / RATE, 1.0),
        "{}",
        sound as f64 / 2.0 / RATE
    );
}

#[test]
fn a_file_played_anew_forgets_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 0.25, 48_000, 2);
    let (handle, _capture) = player(None);
    handle.send(play(&a, 0.0));
    handle.send(Command::Next(Some(b.clone())));
    handle.send(play(&a, 0.0));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert!(!told.contains(&Event::Into(b)));
    assert_eq!(told.last(), Some(&Event::End(a)));
}

#[test]
fn skip_silence_drops_a_long_pause_but_keeps_its_place() {
    let dir = tempfile::tempdir().unwrap();
    // A quarter second of sound, a second of silence, a quarter of sound.
    let sound = |f: u32, _| {
        if (12_000..60_000).contains(&f) {
            0
        } else {
            8192
        }
    };
    let path = wav_of(dir.path(), "a.wav", 1.5, 48_000, 2, sound);
    let (handle, capture) = player(None);
    handle.send(Command::SkipSilence(true));
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    // A quarter second of the pause kept.
    assert!(near(heard(&capture), 0.75), "{}", heard(&capture));
}

#[test]
fn a_boost_plays_louder_and_rounds_off_rather_than_clips() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let loud = wav_of(dir.path(), "b.wav", 0.25, 48_000, 2, |_, _| 30_000);
    let (handle, capture) = player(None);
    handle.send(Command::Boost(2.0));
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    let level = capture.samples.lock().unwrap()[1000];
    assert!((level - 0.25 * 0.794 * 2.0).abs() < 1e-3, "{level}");
    capture.samples.lock().unwrap().clear();
    handle.send(play(&loud, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    let level = capture.samples.lock().unwrap()[1000];
    assert!(
        level > 0.9 && level < 1.0,
        "rounded off below full scale: {level}"
    );
}

#[test]
fn a_crossfade_blends_the_end_into_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 1.0, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 1.0, 48_000, 2);
    let (handle, capture) = player(Some(8.0));
    handle.send(Command::Crossfade(0.5));
    handle.send(play(&a, 0.0));
    handle.send(Command::Next(Some(b.clone())));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert!(told.contains(&Event::Into(b)), "{told:?}");
    let samples = capture.samples.lock().unwrap();
    let sound = samples.iter().rposition(|&s| s != 0.0).unwrap() + 1;
    // Half a second shared: a second and a half in all.
    assert!(
        near(sound as f64 / 2.0 / RATE, 1.5),
        "{}",
        sound as f64 / 2.0 / RATE
    );
}
