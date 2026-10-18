//! The device's sound, through ALSA's library, loaded when slot's player starts as slot's own
//! sound is, so nothing is linked at build time. The device is opened at the first write and
//! closed when the player lets it go: BaseOS has no mixer, so a game can only sound once music
//! has let go of it.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CStr};

use libloading::Library;

use crate::engine::{Output, CHANNELS, RATE};

const STREAM_PLAYBACK: c_int = 0;
const FORMAT_S16_LE: c_int = 2;
const ACCESS_RW_INTERLEAVED: c_int = 3;
/// How much the device holds: plenty to ride out a busy moment and to sleep between writes.
/// A skip, a seek or a pause drops it, so it never delays them.
const LATENCY_US: c_uint = 300_000;
const DEVICE: &CStr = c"default";
/// How long a busy device is waited for, and how often it is tried meanwhile.
const BUSY_FOR: std::time::Duration = std::time::Duration::from_secs(1);
const BUSY_RETRY: std::time::Duration = std::time::Duration::from_millis(20);
/// Linux's "device or resource busy".
const EBUSY: c_int = 16;
/// How long sound fades out where it is cut short, and in where it starts: 10 ms, too short to
/// hear as a fade, long enough not to click.
const FADE: usize = RATE as usize / 100;
/// How much of what was written is kept, to fade out from: all the device can hold.
const KEPT: usize = RATE as usize / 2 * CHANNELS as usize;

type Pcm = *mut c_void;

pub struct Alsa {
    open: unsafe extern "C" fn(*mut Pcm, *const c_char, c_int, c_int) -> c_int,
    set_params: unsafe extern "C" fn(Pcm, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int,
    writei: unsafe extern "C" fn(Pcm, *const c_void, c_ulong) -> c_long,
    recover: unsafe extern "C" fn(Pcm, c_int, c_int) -> c_int,
    delay: unsafe extern "C" fn(Pcm, *mut c_long) -> c_int,
    drop: unsafe extern "C" fn(Pcm) -> c_int,
    drain: unsafe extern "C" fn(Pcm) -> c_int,
    rewindable: unsafe extern "C" fn(Pcm) -> c_long,
    rewind: unsafe extern "C" fn(Pcm, c_ulong) -> c_long,
    prepare: unsafe extern "C" fn(Pcm) -> c_int,
    close: unsafe extern "C" fn(Pcm) -> c_int,
    strerror: unsafe extern "C" fn(c_int) -> *const c_char,
    pcm: Option<Pcm>,
    samples: Vec<i16>,
    /// What was written last, and how many frames are still to fade in.
    written: VecDeque<i16>,
    fade_in: usize,
    dither: Dither,
    _lib: Library,
}

// The device is only ever used from the writer's thread, which owns it.
unsafe impl Send for Alsa {}

impl Alsa {
    pub fn load() -> Result<Alsa, String> {
        let lib = unsafe { Library::new("libasound.so.2") }
            .or_else(|_| unsafe { Library::new("libasound.so") })
            .map_err(|e| format!("libasound: {e}"))?;
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| format!("{}: {e}", $name))?
            };
        }
        unsafe {
            Ok(Alsa {
                open: get!("snd_pcm_open"),
                set_params: get!("snd_pcm_set_params"),
                writei: get!("snd_pcm_writei"),
                recover: get!("snd_pcm_recover"),
                delay: get!("snd_pcm_delay"),
                drop: get!("snd_pcm_drop"),
                drain: get!("snd_pcm_drain"),
                rewindable: get!("snd_pcm_rewindable"),
                rewind: get!("snd_pcm_rewind"),
                prepare: get!("snd_pcm_prepare"),
                close: get!("snd_pcm_close"),
                strerror: get!("snd_strerror"),
                pcm: None,
                samples: Vec::new(),
                written: VecDeque::new(),
                fade_in: 0,
                dither: Dither(0x9e37_79b9),
                _lib: lib,
            })
        }
    }

    fn message(&self, err: c_int) -> String {
        let text = unsafe { (self.strerror)(err) };
        match text.is_null() {
            true => format!("error {err}"),
            false => unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        }
    }

    /// The device, opened if it is not.
    fn pcm(&mut self) -> Result<Pcm, String> {
        if let Some(pcm) = self.pcm {
            return Ok(pcm);
        }
        // slot's own sound lets the device go a moment after music starts; until then it is busy.
        let given_up = std::time::Instant::now() + BUSY_FOR;
        let mut pcm: Pcm = std::ptr::null_mut();
        let err = loop {
            let err = unsafe { (self.open)(&mut pcm, DEVICE.as_ptr(), STREAM_PLAYBACK, 0) };
            if err != -EBUSY || std::time::Instant::now() >= given_up {
                break err;
            }
            std::thread::sleep(BUSY_RETRY);
        };
        if err < 0 || pcm.is_null() {
            return Err(format!("opening the sound: {}", self.message(err)));
        }
        let channels = c_uint::from(CHANNELS);
        let err = unsafe {
            (self.set_params)(
                pcm,
                FORMAT_S16_LE,
                ACCESS_RW_INTERLEAVED,
                channels,
                RATE,
                1,
                LATENCY_US,
            )
        };
        if err < 0 {
            unsafe { (self.close)(pcm) };
            return Err(format!("setting up the sound: {}", self.message(err)));
        }
        self.pcm = Some(pcm);
        self.fade_in = FADE;
        Ok(pcm)
    }
}

impl Alsa {
    /// Ends the sound where it is heard without a click: takes back what the device has not
    /// played, plays a short fade from there, and lets that out. A device that cannot take
    /// sound back is cut short as it is.
    fn fade_out(&mut self, pcm: Pcm) {
        let channels = usize::from(CHANNELS);
        let can = unsafe { (self.rewindable)(pcm) };
        let back = match can > 0 {
            true => unsafe { (self.rewind)(pcm, can as c_ulong) },
            false => 0,
        };
        let back = back.max(0) as usize * channels;
        match back > 0 && back <= self.written.len() {
            true => {
                let from = self.written.len() - back;
                let mut fade: Vec<i16> = self
                    .written
                    .range(from..)
                    .take(FADE * channels)
                    .copied()
                    .collect();
                let frames = fade.len() / channels;
                for (i, frame) in fade.chunks_exact_mut(channels).enumerate() {
                    let down = 1.0 - i as f32 / frames as f32;
                    frame
                        .iter_mut()
                        .for_each(|s| *s = (f32::from(*s) * down) as i16);
                }
                unsafe {
                    (self.writei)(pcm, fade.as_ptr().cast(), frames as c_ulong);
                    (self.drain)(pcm);
                }
            }
            false => unsafe {
                (self.drop)(pcm);
            },
        }
        self.written.clear();
    }
}

impl Output for Alsa {
    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let pcm = self.pcm()?;
        self.samples.clear();
        let dither = &mut self.dither;
        self.samples.extend(samples.iter().map(|&s| dither.cut(s)));
        let channels = usize::from(CHANNELS);
        // A start fades in, rather than jump to where the sound is.
        for (i, frame) in self
            .samples
            .chunks_exact_mut(channels)
            .take(self.fade_in)
            .enumerate()
        {
            let up = (FADE - self.fade_in + i) as f32 / FADE as f32;
            frame
                .iter_mut()
                .for_each(|s| *s = (f32::from(*s) * up) as i16);
        }
        self.fade_in = self.fade_in.saturating_sub(self.samples.len() / channels);
        self.written.extend(&self.samples);
        let over = self.written.len().saturating_sub(KEPT);
        self.written.drain(..over);
        let mut at = 0;
        while at < self.samples.len() {
            let frames = ((self.samples.len() - at) / channels) as c_ulong;
            let ptr = self.samples[at..].as_ptr().cast();
            let n = unsafe { (self.writei)(pcm, ptr, frames) };
            if n < 0 {
                // An underrun after a quiet moment, or a sleep: recover, and write on.
                let err = unsafe { (self.recover)(pcm, n as c_int, 1) };
                if err < 0 {
                    let why = self.message(err);
                    self.release();
                    return Err(format!("writing the sound: {why}"));
                }
                continue;
            }
            at += n as usize * channels;
        }
        Ok(())
    }

    fn delay(&mut self) -> u64 {
        let Some(pcm) = self.pcm else {
            return 0;
        };
        let mut frames: c_long = 0;
        match unsafe { (self.delay)(pcm, &mut frames) } {
            0 => frames.max(0) as u64,
            _ => 0,
        }
    }

    fn flush(&mut self) {
        if let Some(pcm) = self.pcm {
            self.fade_out(pcm);
            unsafe { (self.prepare)(pcm) };
            self.fade_in = FADE;
        }
    }

    fn release(&mut self) {
        if let Some(pcm) = self.pcm.take() {
            self.fade_out(pcm);
            unsafe { (self.close)(pcm) };
        }
    }
}

impl Drop for Alsa {
    fn drop(&mut self) {
        self.release();
    }
}

/// Triangular dither: noise of a step either way, so the quietest sound fades out
/// instead of breaking up as it is cut to 16 bits.
struct Dither(u32);

impl Dither {
    fn cut(&mut self, sample: f32) -> i16 {
        let noise = self.next() - self.next();
        (sample * f32::from(i16::MAX) + noise)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
    }

    /// 0 to 1, from xorshift: noise, not secrets.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1 << 24) as f32
    }
}
