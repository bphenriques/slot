//! The device's sound, through ALSA's library, loaded when slot's player starts as slot's own
//! sound is, so nothing is linked at build time. The device is opened at the first write and
//! closed when the player lets it go: BaseOS has no mixer, so a game can only sound once music
//! has let go of it.

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

type Pcm = *mut c_void;

pub struct Alsa {
    open: unsafe extern "C" fn(*mut Pcm, *const c_char, c_int, c_int) -> c_int,
    set_params: unsafe extern "C" fn(Pcm, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int,
    writei: unsafe extern "C" fn(Pcm, *const c_void, c_ulong) -> c_long,
    recover: unsafe extern "C" fn(Pcm, c_int, c_int) -> c_int,
    delay: unsafe extern "C" fn(Pcm, *mut c_long) -> c_int,
    drop: unsafe extern "C" fn(Pcm) -> c_int,
    prepare: unsafe extern "C" fn(Pcm) -> c_int,
    close: unsafe extern "C" fn(Pcm) -> c_int,
    strerror: unsafe extern "C" fn(c_int) -> *const c_char,
    pcm: Option<Pcm>,
    samples: Vec<i16>,
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
                prepare: get!("snd_pcm_prepare"),
                close: get!("snd_pcm_close"),
                strerror: get!("snd_strerror"),
                pcm: None,
                samples: Vec::new(),
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
        let mut pcm: Pcm = std::ptr::null_mut();
        let err = unsafe { (self.open)(&mut pcm, DEVICE.as_ptr(), STREAM_PLAYBACK, 0) };
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
        Ok(pcm)
    }
}

impl Output for Alsa {
    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let pcm = self.pcm()?;
        self.samples.clear();
        let dither = &mut self.dither;
        self.samples.extend(samples.iter().map(|&s| dither.cut(s)));
        let channels = usize::from(CHANNELS);
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
            unsafe {
                (self.drop)(pcm);
                (self.prepare)(pcm);
            }
        }
    }

    fn release(&mut self) {
        if let Some(pcm) = self.pcm.take() {
            unsafe {
                (self.drop)(pcm);
                (self.close)(pcm);
            }
        }
    }
}

impl Drop for Alsa {
    fn drop(&mut self) {
        self.release();
    }
}

/// Triangular dither, as Muse's: noise of a step either way, so the quietest sound fades out
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
