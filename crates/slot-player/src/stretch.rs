//! A book's speed without its pitch: SoundTouch, built from `soundtouch/` (see `build.rs`),
//! through its C API.

use std::ffi::{c_float, c_int, c_uint, c_void};
use std::ptr::NonNull;

extern "C" {
    fn soundtouch_createInstance() -> *mut c_void;
    fn soundtouch_destroyInstance(h: *mut c_void);
    fn soundtouch_setTempo(h: *mut c_void, tempo: c_float);
    fn soundtouch_setChannels(h: *mut c_void, channels: c_uint) -> c_int;
    fn soundtouch_setSampleRate(h: *mut c_void, rate: c_uint) -> c_int;
    fn soundtouch_putSamples(h: *mut c_void, samples: *const c_float, frames: c_uint) -> c_int;
    fn soundtouch_receiveSamples(h: *mut c_void, out: *mut c_float, frames: c_uint) -> c_uint;
    fn soundtouch_flush(h: *mut c_void) -> c_int;
    fn soundtouch_setSetting(h: *mut c_void, id: c_int, value: c_int) -> c_int;
}

/// SoundTouch's `SETTING_USE_QUICKSEEK`.
const USE_QUICKSEEK: c_int = 2;

/// Interleaved frames in at one tempo, out at another, the pitch kept.
pub struct Stretch {
    st: NonNull<c_void>,
    channels: usize,
}

// SAFETY: SoundTouch keeps no thread's state: an instance may move between threads, and `&mut self`
// methods keep it to one at a time.
unsafe impl Send for Stretch {}

impl Stretch {
    /// For `channels` at `rate`, played `tempo` times as fast (1.5 is half as fast again).
    pub fn new(rate: u32, channels: u16, tempo: f32) -> Stretch {
        // SAFETY: creating an instance takes nothing; it is checked for null before use.
        let st = NonNull::new(unsafe { soundtouch_createInstance() }).expect("SoundTouch");
        // SAFETY: `st` is the live instance just created.
        unsafe {
            soundtouch_setSampleRate(st.as_ptr(), rate);
            soundtouch_setChannels(st.as_ptr(), c_uint::from(channels));
            soundtouch_setTempo(st.as_ptr(), tempo);
            // Seeks where to join the pieces a quicker way: far less work, as good for a voice.
            soundtouch_setSetting(st.as_ptr(), USE_QUICKSEEK, 1);
        }
        Stretch {
            st,
            channels: usize::from(channels),
        }
    }

    /// Takes in `samples`, whole frames.
    pub fn put(&mut self, samples: &[f32]) {
        let frames = (samples.len() / self.channels) as c_uint;
        // SAFETY: `st` is live while `self` is; `samples` holds `frames` whole frames, which it
        // only reads.
        unsafe { soundtouch_putSamples(self.st.as_ptr(), samples.as_ptr(), frames) };
    }

    /// Lets out the rest of what was put in: at the end of a file.
    pub fn flush(&mut self) {
        // SAFETY: `st` is live while `self` is.
        unsafe { soundtouch_flush(self.st.as_ptr()) };
    }

    /// Adds to `out` every frame ready.
    pub fn take(&mut self, out: &mut Vec<f32>) {
        let mut buf = [0.0; 4096];
        let room = (buf.len() / self.channels) as c_uint;
        loop {
            // SAFETY: `st` is live while `self` is; `buf` has room for `room` frames.
            let frames =
                unsafe { soundtouch_receiveSamples(self.st.as_ptr(), buf.as_mut_ptr(), room) };
            if frames == 0 {
                return;
            }
            out.extend_from_slice(&buf[..frames as usize * self.channels]);
        }
    }
}

impl Drop for Stretch {
    fn drop(&mut self) {
        // SAFETY: `st` is live, and is never used again.
        unsafe { soundtouch_destroyInstance(self.st.as_ptr()) };
    }
}
