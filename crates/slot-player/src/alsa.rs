//! The device's sound, through ALSA's library, loaded when slot's player starts as slot's own
//! sound is, so nothing is linked at build time. The device is opened at the first write and
//! closed when the player lets it go: BaseOS has no mixer, so a game can only sound once music
//! has let go of it.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CStr};

use libloading::Library;

use crate::engine::{thread_time, Output, CHANNELS, RATE};

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
/// How long sound fades in where it starts: 10 ms, too short to hear as a fade, long enough not
/// to click.
const FADE: usize = RATE as usize / 100;
/// How long it fades out where it is cut short, a pause or a skip: 30 ms, still far too short to
/// read as lag, and long enough that a small speaker does not thump on the way down.
const FADE_OUT: usize = RATE as usize * 3 / 100;
/// How much of what was written is kept, to fade out from: all the device can hold.
const KEPT: usize = RATE as usize / 2 * CHANNELS as usize;
/// How long a paused device goes on silence before it is let go. The RG SP's speaker
/// amplifier switches off with the device, heard as a click however quietly the sound ended; kept
/// a while, a pause and play again within it is never heard so.
const KEPT_PAUSED: std::time::Duration = std::time::Duration::from_secs(3);
/// How much silence a paused device is given at a time: 50 ms, so playing on waits no longer.
const IDLE: usize = RATE as usize / 20;

type Pcm = *mut c_void;

#[derive(Debug, PartialEq)]
/// Why the device did not open: it failed, or would not take the rate asked.
enum Unopened {
    Failed(String),
    Refused(String),
}

/// Whether ALSA's own resampling is allowed at `rate`: at [`RATE`], as ever; at a file's own,
/// not, so a rate the device does not take is refused, for slot-player's resampler to do.
fn lets_alsa_resample(rate: u32) -> bool {
    rate == RATE
}

/// A device that would not be set up at `rate`: refused at a file's own, which [`RATE`] then
/// stands in for; failed at [`RATE`], for which there is nothing else.
fn unopened(rate: u32, why: String) -> Unopened {
    if lets_alsa_resample(rate) {
        Unopened::Failed(why)
    } else {
        Unopened::Refused(why)
    }
}

/// What of the `can` frames the device holds unplayed to take back, at `rate`: all but the next
/// 50 ms, which it may already be reading ahead of the place it says it is at, its place moving
/// in steps; sound taken back from under it would jump, heard as a click. With less than that
/// unplayed, none: it plays out.
fn taken_back(can: c_long, rate: c_uint) -> c_long {
    let margin = c_long::from(rate) / 20;
    (can - margin).max(0)
}

/// What to take back to fade out from, of the `can` frames the device holds unplayed, at `rate`:
/// as [`taken_back`], but no more than the `kept` frames of it still held to fade from, so the
/// fade starts where the sound was taken back to, never earlier.
fn faded_back(can: c_long, kept: usize, rate: c_uint) -> c_long {
    taken_back(can, rate).min(c_long::try_from(kept).unwrap_or(c_long::MAX))
}

/// How far through a fade of `of` frames frame `i` is, 0 to 1.
fn ramp(i: usize, of: usize) -> f32 {
    (i as f32 / of.max(1) as f32).min(1.0)
}

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
    /// The rate the device is opened at.
    rate: c_uint,
    /// What was written last, and how many frames are still to fade in.
    written: VecDeque<i16>,
    fade_in: usize,
    /// Whether it is paused, going on silence; and the silence it writes.
    hushed: bool,
    silence: Vec<i16>,
    dither: Dither,
    /// This thread's time in ALSA's writing so far, in nanoseconds.
    library: u64,
    _lib: Library,
}

// SAFETY: ALSA's handle may move between threads; it is only ever used from the writer's, which
// owns it.
unsafe impl Send for Alsa {}

impl Alsa {
    pub fn load() -> Result<Alsa, String> {
        // SAFETY: ALSA's library runs nothing of its own as it loads.
        let lib = unsafe { Library::new("libasound.so.2") }
            .or_else(|_| unsafe { Library::new("libasound.so") })
            .map_err(|e| format!("libasound: {e}"))?;
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| format!("{}: {e}", $name))?
            };
        }
        // SAFETY: each symbol is ALSA's function of that name, typed as `alsa/pcm.h` declares
        // it, and kept no longer than `lib`, which is kept with them.
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
                hushed: false,
                silence: Vec::new(),
                rate: RATE,
                dither: Dither(0x9e37_79b9),
                library: 0,
                _lib: lib,
            })
        }
    }

    fn message(&self, err: c_int) -> String {
        // SAFETY: takes any number, and gives a static string or null.
        let text = unsafe { (self.strerror)(err) };
        if text.is_null() {
            format!("error {err}")
        } else {
            // SAFETY: not null, a string ALSA keeps for good, ended by a zero.
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned()
        }
    }

    /// The device, opened if it is not, at the rate set: at [`RATE`] with ALSA's own resampling
    /// allowed, as ever; at the file's own without it, so a rate the device does not take is
    /// refused rather than brought to one by a resampler of unknown quality and cost.
    fn pcm(&mut self) -> Result<Pcm, Unopened> {
        if let Some(pcm) = self.pcm {
            return Ok(pcm);
        }
        // slot's own sound lets the device go a moment after music starts; until then it is busy.
        let given_up = std::time::Instant::now() + BUSY_FOR;
        let mut pcm: Pcm = std::ptr::null_mut();
        let err = loop {
            // SAFETY: `pcm` is somewhere to write the handle, `DEVICE` a string ended by a zero.
            let err = unsafe { (self.open)(&mut pcm, DEVICE.as_ptr(), STREAM_PLAYBACK, 0) };
            if err != -EBUSY || std::time::Instant::now() >= given_up {
                break err;
            }
            std::thread::sleep(BUSY_RETRY);
        };
        if err < 0 || pcm.is_null() {
            return Err(Unopened::Failed(format!(
                "opening the sound: {}",
                self.message(err)
            )));
        }
        let channels = c_uint::from(CHANNELS);
        // SAFETY: `pcm` is the device just opened.
        let err = unsafe {
            (self.set_params)(
                pcm,
                FORMAT_S16_LE,
                ACCESS_RW_INTERLEAVED,
                channels,
                self.rate,
                c_int::from(lets_alsa_resample(self.rate)),
                LATENCY_US,
            )
        };
        if err < 0 {
            // SAFETY: `pcm` is open, and is not used again.
            unsafe { (self.close)(pcm) };
            let why = format!(
                "setting up the sound at {} Hz: {}",
                self.rate,
                self.message(err)
            );
            return Err(unopened(self.rate, why));
        }
        log::info!("sound at {} Hz", self.rate);
        self.pcm = Some(pcm);
        self.fade_in = FADE;
        Ok(pcm)
    }
}

impl Alsa {
    /// Ends the sound where it is heard without a click, and lets the device go quiet: faded out,
    /// played until it is; cut short, dropped.
    fn end(&mut self, pcm: Pcm) {
        let faded = self.fade_out(pcm);
        // SAFETY: `pcm` is the device this has open, as everywhere below that takes it.
        unsafe {
            if faded {
                (self.drain)(pcm)
            } else {
                (self.drop)(pcm)
            }
        };
        self.hushed = false;
    }

    /// Fades the sound out where it is heard: takes back what the device has not played but a
    /// margin it may already be reading, and writes a short fade from there. False where the
    /// device cannot take sound back, and it is to be cut short as it is.
    fn fade_out(&mut self, pcm: Pcm) -> bool {
        let channels = usize::from(CHANNELS);
        // SAFETY: `pcm` is open.
        let can = unsafe { (self.rewindable)(pcm) };
        let take = faded_back(can, self.written.len() / channels, self.rate);
        let back = if take > 0 {
            // SAFETY: `pcm` is open.
            unsafe { (self.rewind)(pcm, take as c_ulong) }
        } else {
            0
        };
        let back = back.max(0) as usize * channels;
        let faded = if back > 0 && back <= self.written.len() {
            let from = self.written.len() - back;
            let mut fade: Vec<i16> = self
                .written
                .range(from..)
                .take(FADE_OUT * channels)
                .copied()
                .collect();
            let frames = fade.len() / channels;
            for (i, frame) in fade.chunks_exact_mut(channels).enumerate() {
                let down = 1.0 - ramp(i, frames);
                frame
                    .iter_mut()
                    .for_each(|s| *s = (f32::from(*s) * down) as i16);
            }
            log::info!("faded out, {} of {can} frames taken back", back / channels);
            // SAFETY: `pcm` is open; `fade` holds `frames` whole frames, which it only reads.
            unsafe { (self.writei)(pcm, fade.as_ptr().cast(), frames as c_ulong) };
            true
        } else if can >= 0 && self.written.len() >= channels {
            // Little left to play: it plays out, then goes down to nothing from its last.
            let last: Vec<i16> = self
                .written
                .range(self.written.len() - channels..)
                .copied()
                .collect();
            let mut fade = Vec::with_capacity(FADE_OUT * channels);
            for i in 0..FADE_OUT {
                let down = 1.0 - ramp(i, FADE_OUT);
                fade.extend(last.iter().map(|&s| (f32::from(s) * down) as i16));
            }
            log::info!("played out, {can} frames left");
            // SAFETY: `pcm` is open; `fade` holds `FADE_OUT` whole frames, which it only reads.
            unsafe { (self.writei)(pcm, fade.as_ptr().cast(), FADE_OUT as c_ulong) };
            true
        } else {
            // Cut where it is: heard as a click, so said, to know how often the device does it.
            log::info!("cut short, {can} frames could be taken back");
            false
        };
        self.written.clear();
        faded
    }
}

impl Output for Alsa {
    fn set_rate(&mut self, rate: u32) -> bool {
        if self.rate == rate {
            return true;
        }
        self.release();
        self.rate = rate;
        // Opened now, to know; any other failure is the next write's to say.
        match self.pcm() {
            Err(Unopened::Refused(why)) => {
                log::info!("{why}");
                self.rate = RATE;
                false
            }
            _ => true,
        }
    }

    fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        let pcm = self.pcm().map_err(|e| match e {
            Unopened::Failed(why) | Unopened::Refused(why) => why,
        })?;
        // Played on from a pause: the silence it went on with is taken back, and the sound fades
        // in after what is left of it.
        if std::mem::take(&mut self.hushed) {
            // SAFETY: `pcm` is open, here and below.
            let take = taken_back(unsafe { (self.rewindable)(pcm) }, self.rate);
            if take > 0 {
                // SAFETY: `pcm` is open.
                unsafe { (self.rewind)(pcm, take as c_ulong) };
            }
            self.fade_in = FADE;
        }
        let mut out = std::mem::take(&mut self.samples);
        out.clear();
        let dither = &mut self.dither;
        out.extend(samples.iter().map(|&s| dither.cut(s)));
        let channels = usize::from(CHANNELS);
        // A start fades in, rather than jump to where the sound is.
        for (i, frame) in out
            .chunks_exact_mut(channels)
            .take(self.fade_in)
            .enumerate()
        {
            let up = ramp(FADE - self.fade_in + i, FADE);
            frame
                .iter_mut()
                .for_each(|s| *s = (f32::from(*s) * up) as i16);
        }
        self.fade_in = self.fade_in.saturating_sub(out.len() / channels);
        let put = self.put(pcm, &out);
        self.samples = out;
        put
    }

    fn delay(&mut self) -> u64 {
        let Some(pcm) = self.pcm else {
            return 0;
        };
        let mut frames: c_long = 0;
        // SAFETY: `pcm` is open; `frames` is somewhere to write the count.
        match unsafe { (self.delay)(pcm, &mut frames) } {
            0 => frames.max(0) as u64,
            _ => 0,
        }
    }

    fn flush(&mut self) {
        if let Some(pcm) = self.pcm {
            self.end(pcm);
            // SAFETY: `pcm` is open.
            unsafe { (self.prepare)(pcm) };
            self.fade_in = FADE;
        }
    }

    fn kept_paused(&self) -> std::time::Duration {
        KEPT_PAUSED
    }

    fn hush(&mut self) {
        let Some(pcm) = self.pcm else {
            return;
        };
        if !self.fade_out(pcm) {
            // SAFETY: `pcm` is open.
            unsafe {
                (self.drop)(pcm);
                (self.prepare)(pcm);
            }
        }
        self.hushed = true;
    }

    fn idle(&mut self) -> Result<(), String> {
        let Some(pcm) = self.pcm.filter(|_| self.hushed) else {
            return Ok(());
        };
        let silence = std::mem::take(&mut self.silence);
        let silence = if silence.is_empty() {
            vec![0; IDLE * usize::from(CHANNELS)]
        } else {
            silence
        };
        let put = self.put(pcm, &silence);
        self.silence = silence;
        put
    }

    fn release(&mut self) {
        if let Some(pcm) = self.pcm.take() {
            self.end(pcm);
            // SAFETY: `pcm` is open, taken out of `self.pcm`, so never used again.
            unsafe { (self.close)(pcm) };
        }
    }

    fn library_time(&self) -> u64 {
        self.library
    }
}

impl Alsa {
    /// Writes `samples`, kept to fade out from, waiting while the device is full.
    fn put(&mut self, pcm: Pcm, samples: &[i16]) -> Result<(), String> {
        self.written.extend(samples);
        let over = self.written.len().saturating_sub(KEPT);
        self.written.drain(..over);
        let began = thread_time();
        let written = self.write_all(pcm, samples);
        self.library += thread_time() - began;
        written
    }

    /// `samples` written whole, by ALSA: recovering from an underrun, failing for good only where
    /// it cannot.
    fn write_all(&mut self, pcm: Pcm, samples: &[i16]) -> Result<(), String> {
        let channels = usize::from(CHANNELS);
        let mut at = 0;
        while at < samples.len() {
            let frames = ((samples.len() - at) / channels) as c_ulong;
            // SAFETY: `pcm` is open; what is left of `samples` holds `frames` whole frames, which
            // it only reads.
            let n = unsafe { (self.writei)(pcm, samples[at..].as_ptr().cast(), frames) };
            if n < 0 {
                // An underrun after a quiet moment, or a sleep: recover, and write on.
                // SAFETY: `pcm` is open.
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
}

impl Drop for Alsa {
    fn drop(&mut self) {
        self.release();
    }
}

/// Triangular dither: noise of a step either way, so the quietest sound fades out
/// instead of breaking up as it is cut to 16 bits.
pub(crate) struct Dither(pub(crate) u32);

impl Dither {
    /// `sample` to 16 bits: the cast saturates, so past full scale is full scale.
    pub(crate) fn cut(&mut self, sample: f32) -> i16 {
        (sample * f32::from(i16::MAX) + self.noise()).round() as i16
    }

    /// -1 to 1, triangular: the difference of two uniform values, the two halves of one draw
    /// from xorshift. Noise, not secrets.
    fn noise(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        let (high, low) = ((self.0 >> 16) as i32, (self.0 & 0xffff) as i32);
        (high - low) as f32 / 65536.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_s_own_rate_is_asked_without_alsa_s_resampling_and_refused_rather_than_failed() {
        assert!(lets_alsa_resample(RATE), "the rate that stands in, as ever");
        assert!(
            !lets_alsa_resample(44_100),
            "a file's own: the device's, or none"
        );
        let why = || "Invalid argument".to_string();
        assert_eq!(
            unopened(44_100, why()),
            Unopened::Refused(why()),
            "48 kHz then"
        );
        assert_eq!(
            unopened(RATE, why()),
            Unopened::Failed(why()),
            "nothing else to try"
        );
    }

    #[test]
    fn sound_is_taken_back_but_for_what_the_device_may_already_be_reading() {
        assert_eq!(taken_back(12_000, 44_100), 12_000 - 2_205, "all but 50 ms");
        assert_eq!(taken_back(2_000, 44_100), 0, "less than that: it plays out");
        assert_eq!(taken_back(-32, 44_100), 0, "an error takes nothing back");
    }

    #[test]
    fn a_fade_out_takes_back_only_what_it_still_holds_to_fade_from() {
        assert_eq!(faded_back(12_000, 20_000, 44_100), 12_000 - 2_205, "as any");
        assert_eq!(
            faded_back(12_000, 4_096, 44_100),
            4_096,
            "a pause just after playing on: the silence before it plays out"
        );
        assert_eq!(faded_back(2_000, 4_096, 44_100), 0);
    }

    #[test]
    fn a_fade_ramps_evenly_from_its_start_to_its_end() {
        assert_eq!(ramp(0, FADE), 0.0);
        assert_eq!(ramp(FADE / 2, FADE), 0.5);
        assert_eq!(ramp(FADE * 2, FADE), 1.0, "never past full");
        assert_eq!(ramp(3, 0), 1.0, "a fade of nothing is done");
    }

    #[test]
    fn dither_is_triangular_a_step_either_way_an_eighth_of_the_time_each() {
        let mut dither = Dither(0x9e37_79b9);
        let mut steps = [0u32; 3];
        for _ in 0..80_000 {
            steps[(dither.cut(0.0) + 1) as usize] += 1;
        }
        // Of 80 000: 10 000 down, 60 000 none, 10 000 up, as a triangle from -1 to 1 rounds.
        for (got, wanted) in steps.into_iter().zip([10_000, 60_000, 10_000]) {
            assert!(got.abs_diff(wanted) < 500, "{steps:?}");
        }
    }

    #[test]
    fn dither_stays_within_a_step_and_never_past_full_scale() {
        let mut dither = Dither(0x9e37_79b9);
        for _ in 0..1000 {
            let quiet = dither.cut(0.0);
            assert!((-1..=1).contains(&quiet), "{quiet}");
            assert_eq!(dither.cut(2.0), i16::MAX);
            assert_eq!(dither.cut(-2.0), i16::MIN);
        }
    }
}
