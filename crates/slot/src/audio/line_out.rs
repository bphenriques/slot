//! The codec's volume, for Muse, which plays straight to it. `lineout volume` is 0 to 31 at
//! 1.5 dB a step, 0 muted, as plorpOS found on the H700.

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
        let lib = unsafe { Library::new("libasound.so.2") }.ok()?;
        unsafe {
            let open: Open = *lib.get(b"snd_ctl_open\0").ok()?;
            let malloc: ValueMalloc = *lib.get(b"snd_ctl_elem_value_malloc\0").ok()?;
            let iface: SetInterface = *lib.get(b"snd_ctl_elem_value_set_interface\0").ok()?;
            let name: SetName = *lib.get(b"snd_ctl_elem_value_set_name\0").ok()?;
            let card = CString::new(CARD).ok()?;
            let control = CString::new(CONTROL).ok()?;
            let mut ctl = std::ptr::null_mut();
            if open(&mut ctl, card.as_ptr(), 0) < 0 {
                return None;
            }
            let close: Close = *lib.get(b"snd_ctl_close\0").ok()?;
            let mut value = std::ptr::null_mut();
            if malloc(&mut value) < 0 {
                close(ctl);
                return None;
            }
            iface(value, IFACE_MIXER);
            name(value, control.as_ptr());
            Some(Ctl {
                ctl,
                value,
                close,
                free: *lib.get(b"snd_ctl_elem_value_free\0").ok()?,
                read: *lib.get(b"snd_ctl_elem_read\0").ok()?,
                write: *lib.get(b"snd_ctl_elem_write\0").ok()?,
                get: *lib.get(b"snd_ctl_elem_value_get_integer\0").ok()?,
                set: *lib.get(b"snd_ctl_elem_value_set_integer\0").ok()?,
                _lib: lib,
            })
        }
    }

    fn read(&self) -> Option<c_long> {
        unsafe { ((self.read)(self.ctl, self.value) >= 0).then(|| (self.get)(self.value, 0)) }
    }

    fn write(&self, raw: c_long) -> bool {
        unsafe {
            (self.set)(self.value, 0, raw);
            (self.write)(self.ctl, self.value) >= 0
        }
    }
}

impl Drop for Ctl {
    fn drop(&mut self) {
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

/// While Muse plays, the codec follows slot's volume; afterwards it is restored.
#[derive(Default)]
pub struct LineOut {
    ctl: Option<Ctl>,
    tried: bool,
    saved: Option<c_long>,
    applied: Option<u8>,
}

impl LineOut {
    /// `Some(volume)` while Muse plays, `None` once slot has the sound back.
    pub fn follow(&mut self, volume: Option<u8>) {
        match volume {
            Some(v) if self.applied != Some(v) => {
                self.open();
                let Some(ctl) = &self.ctl else {
                    return;
                };
                if self.saved.is_none() {
                    self.saved = ctl.read();
                }
                if !ctl.write(raw_level(v)) {
                    eprintln!("slot: audio: {CONTROL} refused");
                }
                self.applied = Some(v);
            }
            None if self.applied.is_some() => {
                if let (Some(raw), Some(ctl)) = (self.saved.take(), &self.ctl) {
                    ctl.write(raw);
                }
                self.applied = None;
            }
            _ => {}
        }
    }

    fn open(&mut self) {
        if !std::mem::replace(&mut self.tried, true) {
            self.ctl = Ctl::open();
            if self.ctl.is_none() {
                eprintln!("slot: audio: no {CONTROL} on {CARD}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_maps_to_the_codec_steps_with_zero_a_mute() {
        assert_eq!(raw_level(0), 0);
        assert_eq!(raw_level(1), RAW_FLOOR);
        assert_eq!(raw_level(100), RAW_TOP);
        assert_eq!(raw_level(200), RAW_TOP);
        assert!((raw_level(50) - 17).abs() <= 1);
    }
}
