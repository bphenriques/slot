//! The codec's volume, for the music player, which plays straight to it. `lineout volume` is 0 to 31 at
//! 1.5 dB a step, 0 muted, as plorpOS found on the H700.

#![warn(clippy::match_bool, clippy::undocumented_unsafe_blocks)]

use std::ffi::{c_char, c_int, c_long, c_uint, c_void, CString};

use libloading::Library;

const CARD: &str = "hw:0";
const CONTROL: &str = "lineout volume";
const IFACE_MIXER: c_int = 2;
const RAW_TOP: c_long = 31;
const RAW_FLOOR: c_long = 3;

type Open = unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int) -> c_int;
type Close = unsafe extern "C" fn(*mut c_void) -> c_int;
type ValueMalloc = unsafe extern "C" fn(*mut *mut c_void) -> c_int;
type ValueFree = unsafe extern "C" fn(*mut c_void);
type SetInterface = unsafe extern "C" fn(*mut c_void, c_int);
type SetName = unsafe extern "C" fn(*mut c_void, *const c_char);
type Elem = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type GetInteger = unsafe extern "C" fn(*const c_void, c_uint) -> c_long;
type SetInteger = unsafe extern "C" fn(*mut c_void, c_uint, c_long);

struct Ctl {
    ctl: *mut c_void,
    value: *mut c_void,
    close: Close,
    free: ValueFree,
    read: Elem,
    write: Elem,
    get: GetInteger,
    set: SetInteger,
    _lib: Library,
}

impl Ctl {
    fn open() -> Option<Ctl> {
        // SAFETY: ALSA's library runs nothing of its own as it loads.
        let lib = unsafe { Library::new("libasound.so.2") }.ok()?;
        // SAFETY: each symbol is ALSA's function of that name, typed as `alsa/control.h` declares
        // it, and kept no longer than `lib`, which is kept with them. All are found before the
        // device is opened, so a missing one leaves nothing open.
        let (open, malloc, iface, name, close, free, read, write, get, set) = unsafe {
            (
                *lib.get::<Open>(b"snd_ctl_open\0").ok()?,
                *lib.get::<ValueMalloc>(b"snd_ctl_elem_value_malloc\0")
                    .ok()?,
                *lib.get::<SetInterface>(b"snd_ctl_elem_value_set_interface\0")
                    .ok()?,
                *lib.get::<SetName>(b"snd_ctl_elem_value_set_name\0").ok()?,
                *lib.get::<Close>(b"snd_ctl_close\0").ok()?,
                *lib.get::<ValueFree>(b"snd_ctl_elem_value_free\0").ok()?,
                *lib.get::<Elem>(b"snd_ctl_elem_read\0").ok()?,
                *lib.get::<Elem>(b"snd_ctl_elem_write\0").ok()?,
                *lib.get::<GetInteger>(b"snd_ctl_elem_value_get_integer\0")
                    .ok()?,
                *lib.get::<SetInteger>(b"snd_ctl_elem_value_set_integer\0")
                    .ok()?,
            )
        };
        let card = CString::new(CARD).ok()?;
        let control = CString::new(CONTROL).ok()?;
        let mut ctl = std::ptr::null_mut();
        // SAFETY: `ctl` is somewhere to write the handle; `card` a string ended by a zero.
        if unsafe { open(&mut ctl, card.as_ptr(), 0) } < 0 {
            return None;
        }
        let mut value = std::ptr::null_mut();
        // SAFETY: `value` is somewhere to write the new value's pointer.
        if unsafe { malloc(&mut value) } < 0 {
            // SAFETY: `ctl` is open, and is not used again.
            unsafe { close(ctl) };
            return None;
        }
        // SAFETY: `value` was just made; `control` is a string ended by a zero, which ALSA copies.
        unsafe {
            iface(value, IFACE_MIXER);
            name(value, control.as_ptr());
        }
        Some(Ctl {
            ctl,
            value,
            close,
            free,
            read,
            write,
            get,
            set,
            _lib: lib,
        })
    }

    fn read(&self) -> Option<c_long> {
        // SAFETY: `ctl` is open and `value` made, while `self` is.
        unsafe { ((self.read)(self.ctl, self.value) >= 0).then(|| (self.get)(self.value, 0)) }
    }

    fn write(&self, raw: c_long) -> bool {
        // SAFETY: `ctl` is open and `value` made, while `self` is.
        unsafe {
            (self.set)(self.value, 0, raw);
            (self.write)(self.ctl, self.value) >= 0
        }
    }
}

impl Drop for Ctl {
    fn drop(&mut self) {
        // SAFETY: `value` and `ctl` are live, and are never used again.
        unsafe {
            (self.free)(self.value);
            (self.close)(self.ctl);
        }
    }
}

/// Slot's volume (0 to 100) as a codec step: 0 mutes, 1 to 100 spread from floor to full.
pub fn raw_level(volume: u8) -> c_long {
    match c_long::from(volume.min(100)) {
        0 => 0,
        v => RAW_FLOOR + ((v - 1) * (RAW_TOP - RAW_FLOOR) + 49) / 99,
    }
}

/// What the codec's volume is to do, as music plays at one, or has let it go.
#[derive(Debug, PartialEq)]
enum Change {
    /// Set to music's, the level before kept the first time.
    Music(u8),
    /// Back to the level before music.
    Restore,
}

/// The change from `applied`, what music last set, to `wanted`: none while it stands.
fn change(applied: Option<u8>, wanted: Option<u8>) -> Option<Change> {
    match (applied, wanted) {
        (was, Some(v)) if was != Some(v) => Some(Change::Music(v)),
        (Some(_), None) => Some(Change::Restore),
        _ => None,
    }
}

/// While music plays, the codec follows slot's volume; afterwards it is restored.
#[derive(Default)]
pub struct LineOut {
    ctl: Option<Ctl>,
    tried: bool,
    saved: Option<c_long>,
    applied: Option<u8>,
}

impl LineOut {
    /// `Some(volume)` while music plays, `None` once slot has the sound back.
    pub fn follow(&mut self, volume: Option<u8>) {
        match change(self.applied, volume) {
            Some(Change::Music(v)) => {
                self.open();
                let Some(ctl) = &self.ctl else {
                    return;
                };
                if self.saved.is_none() {
                    self.saved = ctl.read();
                }
                if !ctl.write(raw_level(v)) {
                    log::warn!(target: "slot_audio", "{CONTROL} refused");
                }
                self.applied = Some(v);
            }
            Some(Change::Restore) => {
                if let (Some(raw), Some(ctl)) = (self.saved.take(), &self.ctl) {
                    ctl.write(raw);
                }
                self.applied = None;
            }
            None => {}
        }
    }

    fn open(&mut self) {
        if !std::mem::replace(&mut self.tried, true) {
            self.ctl = Ctl::open();
            if self.ctl.is_none() {
                log::warn!(target: "slot_audio", "no {CONTROL} on {CARD}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codec_follows_music_while_it_plays_then_goes_back_once() {
        assert_eq!(
            change(None, Some(50)),
            Some(Change::Music(50)),
            "music starts"
        );
        assert_eq!(change(Some(50), Some(50)), None, "nothing while it stands");
        assert_eq!(
            change(Some(50), Some(60)),
            Some(Change::Music(60)),
            "a volume key"
        );
        assert_eq!(
            change(Some(60), None),
            Some(Change::Restore),
            "music let it go"
        );
        assert_eq!(change(None, None), None, "slot's, and staying so");
    }

    #[test]
    fn volume_maps_to_the_codec_steps_with_zero_a_mute() {
        assert_eq!(raw_level(0), 0);
        assert_eq!(raw_level(1), RAW_FLOOR);
        assert_eq!(raw_level(100), RAW_TOP);
        assert_eq!(raw_level(200), RAW_TOP);
        assert!((raw_level(50) - 17).abs() <= 1);
    }
}
