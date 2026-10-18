//! slot's player, heard through an output that keeps what it is given: WAV files made here,
//! played, paused, moved through and stopped.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slot_player::{Command, Event, Handle, Output, PlayState, Status};

const WAIT: Duration = Duration::from_secs(5);

/// An output that keeps what it is given: as fast as it is written, or no faster than `pace`
/// times as heard; at any rate it is set to, or only `takes`, as a device that has one; and the
/// rates it was asked for.
#[derive(Clone, Default)]
struct Capture {
    samples: Arc<Mutex<Vec<f32>>>,
    pace: Option<f64>,
    takes: Option<u32>,
    asked: Arc<Mutex<Vec<u32>>>,
}

impl Capture {
    /// The rate it plays at: the last it took.
    fn rate(&self) -> f64 {
        let asked = self.asked.lock().unwrap();
        let took = asked
            .iter()
            .rev()
            .find(|&&r| self.takes.is_none_or(|t| t == r));
        f64::from(took.copied().unwrap_or(48_000))
    }

    fn asked(&self) -> Vec<u32> {
        self.asked.lock().unwrap().clone()
    }
}

impl Output for Capture {
    fn set_rate(&mut self, rate: u32) -> bool {
        self.asked.lock().unwrap().push(rate);
        self.takes.is_none_or(|t| t == rate)
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if let Some(pace) = self.pace {
            let secs = samples.len() as f64 / 2.0 / self.rate() / pace;
            std::thread::sleep(Duration::from_secs_f64(secs));
        }
        self.samples.lock().unwrap().extend_from_slice(samples);
        Ok(())
    }

    fn delay(&mut self) -> u64 {
        0
    }

    fn flush(&mut self) {}

    fn release(&mut self) {}
}

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

/// The status once it is as `wanted`, failing if it is not in time.
fn settle(handle: &Handle, wanted: impl Fn(&Status) -> bool) -> Status {
    let given_up = std::time::Instant::now() + WAIT;
    loop {
        let status = handle.status();
        if wanted(&status) {
            return status;
        }
        assert!(
            std::time::Instant::now() < given_up,
            "not in time: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The seconds of sound the capture holds.
fn heard(capture: &Capture) -> f64 {
    capture.samples.lock().unwrap().len() as f64 / 2.0 / capture.rate()
}

fn near(a: f64, b: f64) -> bool {
    // A chunk of slack: the last is filled out with silence.
    (a - b).abs() < 0.1
}

#[test]
fn a_file_plays_to_its_end_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    // Mono at 44.1 kHz: played at its own rate, as stereo.
    let path = wav(dir.path(), "a.wav", 0.5, 44_100, 1);
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    let told = until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(told, [Event::End(path)], "the end, and nothing else told");
    settle(&handle, |s| s.state == PlayState::Stopped);
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
    let at = settle(&handle, |s| s.state == PlayState::Paused).at;
    assert!(at > 0.2 && at < 1.5, "paused a way in: {at}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(handle.status().at, at, "and stays there");
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

/// An output that plays as `Capture` does and keeps what it was told to do with the device.
#[derive(Clone, Default)]
struct Told {
    capture: Capture,
    told: Arc<Mutex<Vec<&'static str>>>,
}

impl Told {
    fn tell(&self, what: &'static str) {
        let mut told = self.told.lock().unwrap();
        // Silence is given again and again: once said is enough.
        if told.last() != Some(&what) {
            told.push(what);
        }
    }

    fn told(&self) -> Vec<&'static str> {
        self.told.lock().unwrap().clone()
    }
}

impl Output for Told {
    fn set_rate(&mut self, rate: u32) -> bool {
        self.capture.set_rate(rate)
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        self.tell("write");
        self.capture.write(samples)
    }

    fn delay(&mut self) -> u64 {
        0
    }

    fn flush(&mut self) {
        self.tell("flush");
    }

    fn hush(&mut self) {
        self.tell("hush");
    }

    fn idle(&mut self) -> Result<(), String> {
        self.tell("idle");
        Ok(())
    }

    fn release(&mut self) {
        self.tell("release");
    }
}

#[test]
fn a_pause_keeps_the_device_going_on_silence_a_while_so_it_is_not_heard_letting_go() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 10.0, 48_000, 2);
    let told = Told {
        capture: Capture {
            pace: Some(4.0),
            ..Capture::default()
        },
        ..Told::default()
    };
    let handle = Handle::on(told.clone());
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(100));
    handle.send(Command::Pause);
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(
        told.told(),
        ["write", "hush", "idle"],
        "kept past the moment an idle device is let go"
    );
    assert!(handle.status().sounding, "still the music's");
    handle.send(Command::Resume);
    std::thread::sleep(Duration::from_millis(100));
    handle.send(Command::Pause);
    let given_up = std::time::Instant::now() + WAIT;
    while told.told().last() != Some(&"release") {
        assert!(std::time::Instant::now() < given_up, "{:?}", told.told());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        told.told(),
        ["write", "hush", "idle", "write", "hush", "idle", "release"],
        "played on in the device kept, and let go once paused a while"
    );
    assert!(!handle.status().sounding);
}

#[test]
fn a_move_while_paused_plays_on_from_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, capture) = player(Some(4.0));
    handle.send(play(&path, 0.0));
    std::thread::sleep(Duration::from_millis(50));
    handle.send(Command::Pause);
    settle(&handle, |s| s.state == PlayState::Paused);
    std::thread::sleep(Duration::from_millis(50));
    let before = heard(&capture);
    handle.send(Command::Seek(1.5));
    settle(&handle, |s| near(s.at, 1.5));
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
    settle(&handle, |s| s.state == PlayState::Stopped && s.len == 0.0);
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
    fn set_rate(&mut self, _: u32) -> bool {
        true
    }

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
fn a_device_that_will_not_sound_pauses_where_it_was_and_is_no_file_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let handle = Handle::on(Broken);
    handle.send(play(&path, 0.0));
    let told = until(&handle, |e| matches!(e, Event::Lost(_)));
    assert_eq!(told.last(), Some(&Event::Lost("no sound".into())));
    assert!(!told.iter().any(|e| matches!(e, Event::Error(_))));
    settle(&handle, |s| s.state == PlayState::Paused);
}

/// An output that panics: a fault in the player's own code.
struct Panics;

impl slot_player::Output for Panics {
    fn set_rate(&mut self, _: u32) -> bool {
        true
    }

    fn write(&mut self, _: &[f32]) -> Result<(), String> {
        panic!("a fault")
    }

    fn delay(&mut self) -> u64 {
        0
    }

    fn flush(&mut self) {}

    fn release(&mut self) {}
}

#[test]
fn a_player_that_panics_says_it_is_gone_rather_than_play_on_unheard() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let handle = Handle::on(Panics);
    handle.send(play(&path, 0.0));
    let told = until(&handle, |e| *e == Event::Gone);
    assert!(told.iter().any(|e| matches!(e, Event::Lost(_))), "{told:?}");
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
    ups as f64 / (left.len() as f64 / capture.rate())
}

#[test]
fn sound_keeps_its_headroom() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    let level = capture.samples.lock().unwrap()[1000];
    assert!((level - 0.25 * 0.794).abs() < 1e-3, "2 dB down: {level}");
}

#[test]
fn a_file_at_a_rate_the_device_will_not_take_is_resampled_its_pitch_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav_of(dir.path(), "a.wav", 1.0, 44_100, 1, tone(1000.0, 44_100));
    let capture = Capture {
        takes: Some(48_000),
        ..Capture::default()
    };
    let handle = Handle::on(capture.clone());
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
    std::thread::sleep(Duration::from_millis(20));
    let at = handle.status().at;
    assert!(at > 0.2 && at < 1.0, "on from where it was: {at}");
    for _ in 0..20 {
        let status = handle.status();
        assert!(
            status.at <= status.len.max(2.0),
            "never past the end: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    until(&handle, |e| matches!(e, Event::End(_)));
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
        near(sound as f64 / 2.0 / capture.rate(), 1.0),
        "{}",
        sound as f64 / 2.0 / capture.rate()
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
        near(sound as f64 / 2.0 / capture.rate(), 1.5),
        "{}",
        sound as f64 / 2.0 / capture.rate()
    );
}

#[test]
fn a_cued_file_waits_paused_with_nothing_heard() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let (handle, capture) = player(None);
    handle.send(Command::Cue {
        path: path.clone(),
        at: 0.0,
        speed: 1.0,
    });
    let cued = settle(&handle, |s| s.state == PlayState::Paused && s.len > 0.0);
    assert_eq!(cued.at, 0.0, "open, at its start");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(heard(&capture), 0.0, "nothing until played");
    handle.send(Command::Resume);
    until(&handle, |e| matches!(e, Event::End(_)));
    assert!(near(heard(&capture), 0.25));
}

#[test]
fn the_status_moves_on_with_what_is_heard_and_holds_still_paused() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let (handle, _capture) = player(Some(1.0));
    handle.send(play(&path, 0.0));
    let first = settle(&handle, |s| s.state == PlayState::Playing && s.at > 0.0);
    assert!(near(first.len, 2.0), "its length: {first:?}");
    std::thread::sleep(Duration::from_millis(300));
    let later = handle.status();
    let moved = later.at - first.at;
    assert!(
        moved > 0.15 && moved < 0.6,
        "about as long as it played: {moved}"
    );
    assert!(later.sounding, "the device heard");
    // Paused, it is quiet only once the device has faded out and been let go.
    handle.send(Command::Pause);
    let quiet = settle(&handle, |s| !s.sounding);
    assert_eq!(quiet.state, PlayState::Paused);
    handle.send(Command::Stop);
}

#[test]
fn a_file_plays_at_its_own_rate_where_the_device_takes_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav_of(dir.path(), "a.wav", 1.0, 44_100, 1, tone(1000.0, 44_100));
    let (handle, capture) = player(None);
    handle.send(play(&path, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(capture.asked(), [44_100], "no resampling");
    assert!(near(heard(&capture), 1.0), "{}", heard(&capture));
    assert!((pitch(&capture) - 1000.0).abs() < 5.0);
}

#[test]
fn a_rate_refused_plays_on_at_the_device_s_from_where_it_was_and_is_not_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 2.0, 44_100, 2);
    let b = wav(dir.path(), "b.wav", 0.25, 44_100, 2);
    let capture = Capture {
        takes: Some(48_000),
        pace: Some(1.0),
        ..Capture::default()
    };
    let handle = Handle::on(capture.clone());
    handle.send(play(&a, 0.5));
    let status = settle(&handle, |s| s.state == PlayState::Playing && s.at > 0.6);
    assert!(status.at < 1.0, "from where it was asked: {status:?}");
    handle.send(play(&b, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(capture.asked(), [44_100, 48_000], "refused once");
}

#[test]
fn a_file_gone_on_into_takes_the_rate_playing_and_a_new_one_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 0.25, 44_100, 2);
    let (handle, capture) = player(None);
    handle.send(play(&a, 0.0));
    handle.send(Command::Next(Some(b.clone())));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(capture.asked(), [48_000], "no gap for a change of rate");
    assert!(near(heard(&capture), 0.5), "{}", heard(&capture));
    handle.send(play(&b, 0.0));
    until(&handle, |e| matches!(e, Event::End(_)));
    assert_eq!(capture.asked(), [48_000, 44_100], "played anew, at its own");
}

/// A device as a real one is: it plays what it holds as time passes, holds at most `HOLDS`
/// seconds, so a write waits while it is full, and drops what it holds when flushed or let go.
/// Its clock says what has truly been heard, to hold the status to.
#[derive(Clone, Default)]
struct Device(Arc<Mutex<Played>>);

#[derive(Default)]
struct Played {
    rate: u32,
    /// Since when it has been playing, what it was given since, and what it played before that.
    since: Option<std::time::Instant>,
    given: u64,
    before: u64,
    /// Every frame truly heard, across flushes; and whether it was let go of for good.
    heard: u64,
    gone: bool,
}

const HOLDS: f64 = 0.3;

impl Played {
    fn played(&self) -> u64 {
        let Some(since) = self.since else {
            return self.before;
        };
        let by_now = (since.elapsed().as_secs_f64() * f64::from(self.rate)) as u64;
        self.before + by_now.min(self.given)
    }

    /// Stops where it is, dropping what it holds.
    fn stop(&mut self) {
        let played = self.played();
        self.heard += played - self.before;
        (self.since, self.given, self.before) = (None, 0, 0);
    }
}

impl Device {
    /// Seconds truly heard, across every stop.
    fn heard(&self) -> f64 {
        let p = self.0.lock().unwrap();
        (p.heard + p.played() - p.before) as f64 / f64::from(p.rate.max(1))
    }
}

impl Output for Device {
    fn set_rate(&mut self, rate: u32) -> bool {
        let mut p = self.0.lock().unwrap();
        p.stop();
        p.rate = rate;
        true
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let frames = (samples.len() / 2) as u64;
        loop {
            let mut p = self.0.lock().unwrap();
            let room = (HOLDS * f64::from(p.rate)) as u64;
            if p.given - (p.played() - p.before) + frames <= room {
                p.since.get_or_insert_with(std::time::Instant::now);
                p.given += frames;
                return Ok(());
            }
            drop(p);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn delay(&mut self) -> u64 {
        let p = self.0.lock().unwrap();
        p.given - (p.played() - p.before)
    }

    fn flush(&mut self) {
        self.0.lock().unwrap().stop();
    }

    fn release(&mut self) {
        self.0.lock().unwrap().stop();
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // The player's own copy going is the player letting go of the device.
        if Arc::strong_count(&self.0) == 2 {
            self.0.lock().unwrap().gone = true;
        }
    }
}

#[test]
fn the_place_said_is_what_the_device_has_played_never_ahead_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let device = Device::default();
    let handle = Handle::on(device.clone());
    handle.send(play(&path, 0.0));
    settle(&handle, |s| s.state == PlayState::Playing && s.at > 0.0);
    // Sampled through more than a dozen chunks written, each a wait while the device is full.
    let mut worst: f64 = 0.0;
    for _ in 0..60 {
        let (said, truth) = (handle.status().at, device.heard());
        worst = worst.max((said - truth).abs());
        std::thread::sleep(Duration::from_millis(17));
    }
    assert!(worst < 0.02, "off by {worst:.3} s at most");
    handle.send(Command::Stop);
}

#[test]
fn going_on_into_the_next_the_place_never_passes_the_end_nor_goes_back() {
    let dir = tempfile::tempdir().unwrap();
    let a = wav(dir.path(), "a.wav", 0.4, 48_000, 2);
    let b = wav(dir.path(), "b.wav", 0.6, 48_000, 2);
    let handle = Handle::on(Device::default());
    handle.send(play(&a, 0.0));
    handle.send(Command::Next(Some(b.clone())));
    let mut seen: Vec<Status> = Vec::new();
    let mut told = Vec::new();
    let given_up = std::time::Instant::now() + WAIT;
    while !told.iter().any(|e| matches!(e, Event::End(_))) {
        assert!(std::time::Instant::now() < given_up, "{told:?}");
        seen.push(handle.status());
        told.extend(handle.events());
        std::thread::sleep(Duration::from_millis(3));
    }
    assert_eq!(told, [Event::Into(b.clone()), Event::End(b)]);
    for s in seen.iter().filter(|s| s.state == PlayState::Playing) {
        assert!(s.at <= s.len + 1e-9, "past the end: {s:?}");
    }
    for pair in seen.windows(2) {
        let same = pair[0].len == pair[1].len && pair[1].state == PlayState::Playing;
        assert!(!same || pair[1].at >= pair[0].at - 1e-9, "back: {pair:?}");
    }
}

#[test]
fn commands_act_in_the_order_sent_a_seek_then_a_pause_pauses_there() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 3.0, 48_000, 2);
    let handle = Handle::on(Device::default());
    handle.send(play(&path, 0.0));
    settle(&handle, |s| s.state == PlayState::Playing && s.at > 0.0);
    handle.send(Command::Seek(2.0));
    handle.send(Command::Pause);
    let paused = settle(&handle, |s| s.state == PlayState::Paused);
    assert!((paused.at - 2.0).abs() < 0.05, "{paused:?}");
    assert!(
        !settle(&handle, |s| !s.sounding).sounding,
        "quiet once let go"
    );
}

#[test]
fn a_player_let_go_ends_and_lets_its_device_go() {
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 2.0, 48_000, 2);
    let device = Device::default();
    let handle = Handle::on(device.clone());
    handle.send(play(&path, 0.0));
    settle(&handle, |s| s.state == PlayState::Playing && s.at > 0.0);
    drop(handle);
    let given_up = std::time::Instant::now() + WAIT;
    while !device.0.lock().unwrap().gone {
        assert!(std::time::Instant::now() < given_up, "still held");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Writes `items` into `path`'s tags, as a tagger would.
fn tag(path: &Path, items: &[(lofty::tag::ItemKey, &str)]) {
    use lofty::prelude::*;
    let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
    for (key, value) in items {
        tag.insert_text(*key, (*value).into());
    }
    tag.save_to_path(path, lofty::config::WriteOptions::default())
        .unwrap();
}

#[test]
fn leveling_plays_a_song_at_its_replaygain_and_off_as_recorded() {
    use lofty::tag::ItemKey;
    let dir = tempfile::tempdir().unwrap();
    let path = wav(dir.path(), "a.wav", 0.25, 48_000, 2);
    tag(
        &path,
        &[
            (ItemKey::ReplayGainTrackGain, "-6.02 dB"),
            (ItemKey::ReplayGainAlbumGain, "-12.04 dB"),
        ],
    );
    let level = |leveling| {
        let (handle, capture) = player(None);
        handle.send(Command::Level(leveling));
        handle.send(play(&path, 0.0));
        until(&handle, |e| matches!(e, Event::End(_)));
        let level = capture.samples.lock().unwrap()[1000];
        level / (0.25 * 0.794)
    };
    assert!(
        (level(slot_player::Leveling::Off) - 1.0).abs() < 1e-2,
        "as recorded"
    );
    assert!(
        (level(slot_player::Leveling::Track) - 0.5).abs() < 1e-2,
        "the song's"
    );
    assert!(
        (level(slot_player::Leveling::Album) - 0.25).abs() < 1e-2,
        "the album's"
    );
}
