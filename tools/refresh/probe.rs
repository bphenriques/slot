// Linux/AArch64: rustc -O probe.rs -o probe. Read-only process memory; no ptrace.
// sample PID gpsp|gba|gb SECONDS; response /dev/input/eventN COUNT TRACEFILE
use std::convert::{TryFrom, TryInto};
use std::ffi::c_int;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::FileExt,
    thread::sleep,
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn sysconf(name: c_int) -> i64;
    fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
}
#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}
fn monotonic_ns() -> Result<u64> {
    let mut t = Timespec { sec: 0, nsec: 0 };
    if unsafe { clock_gettime(1, &mut t) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(u64::try_from(t.sec)? * 1_000_000_000 + u64::try_from(t.nsec)?)
}
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn bad(s: &str) -> Box<dyn std::error::Error> {
    io::Error::other(s).into()
}
fn u16at(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2).ok_or("short ELF")?.try_into()?,
    ))
}
fn u32at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4).ok_or("short ELF")?.try_into()?,
    ))
}
fn u64at(b: &[u8], p: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        b.get(p..p + 8).ok_or("short ELF")?.try_into()?,
    ))
}
struct Symbol {
    value: u64,
    size: u64,
    section: usize,
}
struct Elf {
    data: Vec<u8>,
    sh: usize,
    stride: usize,
    count: usize,
}
impl Elf {
    fn open(path: &str) -> Result<Self> {
        let b = fs::read(path)?;
        if b.get(..6) != Some(b"\x7fELF\x02\x01") {
            return Err(bad("need little-endian ELF64"));
        }
        if u16at(&b, 18)? != 183 {
            return Err(bad("need AArch64 core"));
        }
        let e = Self {
            sh: u64at(&b, 40)? as usize,
            stride: u16at(&b, 58)? as usize,
            count: u16at(&b, 60)? as usize,
            data: b,
        };
        if e.stride < 64 {
            return Err(bad("invalid sections"));
        }
        Ok(e)
    }
    fn symbol(&self, name: &str) -> Result<Symbol> {
        let b = &self.data;
        for i in 0..self.count {
            let p = self.sh + i * self.stride;
            if u32at(b, p + 4)? != 2 {
                continue;
            }
            let strings = self.sh + u32at(b, p + 40)? as usize * self.stride;
            let start = u64at(b, strings + 24)? as usize;
            let off = u64at(b, p + 24)? as usize;
            let len = u64at(b, p + 32)? as usize;
            let step = u64at(b, p + 56)? as usize;
            if step < 24 {
                return Err(bad("invalid symbol stride"));
            }
            for j in (off..off + len).step_by(step) {
                let n = start + u32at(b, j)? as usize;
                let tail = b.get(n..).ok_or("invalid symbol name")?;
                let end = tail
                    .iter()
                    .position(|&x| x == 0)
                    .ok_or("unterminated name")?;
                if &tail[..end] == name.as_bytes() {
                    return Ok(Symbol {
                        value: u64at(b, j + 8)?,
                        size: u64at(b, j + 16)?,
                        section: u16at(b, j + 6)? as usize,
                    });
                }
            }
        }
        Err(bad(&format!("missing .symtab symbol {name}")))
    }
    fn bytes(&self, s: &Symbol) -> Result<&[u8]> {
        let p = self.sh + s.section * self.stride;
        let off = u64at(&self.data, p + 24)? + s.value - u64at(&self.data, p + 16)?;
        self.data
            .get(off as usize..(off + s.size) as usize)
            .ok_or_else(|| bad("symbol outside file"))
    }
}
fn read<const N: usize>(f: &File, off: u64) -> Result<[u8; N]> {
    let mut b = [0; N];
    f.read_exact_at(&mut b, off)?;
    Ok(b)
}
enum Counter {
    Direct(u64),
    Board { pointer: u64, offset: u64 },
}
impl Counter {
    fn open(elf: &Elf, base: u64, kind: &str) -> Result<Self> {
        if kind == "gpsp" {
            let s = elf.symbol("frame_counter")?;
            if s.size != 4 {
                return Err(bad("unexpected gpSP frame_counter size"));
            }
            return Ok(Self::Direct(base + s.value));
        }
        let core = elf.symbol("core")?;
        if core.size != 8 {
            return Err(bad("unexpected mGBA core pointer size"));
        }
        let name = if kind == "gba" {
            "_GBACoreFrameCounter"
        } else {
            "_GBCoreFrameCounter"
        };
        let s = elf.symbol(name)?;
        let b = elf.bytes(&s)?;
        if b.len() != 12 || u32at(b, 0)? != 0xf9400400 || u32at(b, 8)? != 0xd65f03c0 {
            return Err(bad("unrecognized frame accessor"));
        }
        let load = u32at(b, 4)?;
        if load & !0x003ffc00 != 0xb9400000 {
            return Err(bad("expected ldr w0,[x0,#imm]"));
        }
        let offset = ((load >> 10) & 4095) * 4;
        if kind == "gba" && offset != 5440 {
            return Err(bad("unexpected GBA frame offset"));
        }
        Ok(Self::Board {
            pointer: base + core.value,
            offset: u64::from(offset),
        })
    }
    fn read(&self, f: &File) -> Result<u32> {
        let (pointer, offset) = match *self {
            Self::Direct(addr) => return Ok(u32::from_le_bytes(read(f, addr)?)),
            Self::Board { pointer, offset } => (pointer, offset),
        };
        let ptr = u64::from_le_bytes(read(f, pointer)?);
        let board = u64::from_le_bytes(read(f, ptr + 8)?);
        if ptr == 0 || board == 0 {
            return Err(bad("null mGBA core/board"));
        }
        Ok(u32::from_le_bytes(read(f, board + offset)?))
    }
}
fn number(path: &str) -> Result<u64> {
    Ok(fs::read_to_string(path)?.trim().parse()?)
}
fn lcd() -> Result<u64> {
    let s = fs::read_to_string("/proc/interrupts")?;
    let l = s
        .lines()
        .find(|l| l.trim_start().starts_with("293:"))
        .ok_or("LCD IRQ 293 absent")?;
    Ok(l.split_whitespace()
        .skip(1)
        .take_while(|v| v.chars().all(|c| c.is_ascii_digit()))
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .sum())
}
fn sample(pid: &str, kind: &str, secs: f64) -> Result<()> {
    if !["gpsp", "gba", "gb"].contains(&kind) || !secs.is_finite() || secs <= 0. {
        return Err(bad("invalid kind/duration"));
    }
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))?;
    let needle = if kind == "gpsp" {
        "gpsp_libretro.so"
    } else {
        "mgba_libretro.so"
    };
    let line = maps
        .lines()
        .find(|l| l.contains(needle) && l.split_whitespace().nth(2) == Some("00000000"))
        .ok_or("core mapping missing")?;
    let fields: Vec<_> = line.split_whitespace().collect();
    let base = u64::from_str_radix(fields[0].split('-').next().unwrap(), 16)?;
    let elf = Elf::open(fields.get(5).ok_or("missing core path")?)?;
    let mem = File::open(format!("/proc/{pid}/mem"))?;
    let counter = Counter::open(&elf, base, kind)?;
    let first = counter.read(&mem)?;
    let began = Instant::now();
    let mut changed = false;
    let tick_hz = unsafe { sysconf(2) };
    if tick_hz <= 0 {
        return Err(bad("cannot read Linux _SC_CLK_TCK"));
    }
    println!(
        "monotonic_secs,committed_frames,cpu_ticks,clock_khz,temp_millic,lcd_irqs,cpu_tick_hz"
    );
    let mut target = began;
    loop {
        let elapsed = began.elapsed().as_secs_f64();
        let frame = counter.read(&mem)?;
        let frames = i64::from(frame) - i64::from(first);
        changed |= frames > 0;
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let tail = stat.rsplit_once(')').ok_or("invalid proc stat")?.1;
        let v: Vec<_> = tail.split_whitespace().collect();
        let ticks = v.get(11).ok_or("short proc stat")?.parse::<u64>()?
            + v.get(12).ok_or("short proc stat")?.parse::<u64>()?;
        println!(
            "{elapsed:.6},{frames},{ticks},{},{},{},{tick_hz}",
            number("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")?,
            number("/sys/class/thermal/thermal_zone0/temp")?,
            lcd()?
        );
        if elapsed >= secs {
            break;
        }
        target += Duration::from_millis(100);
        sleep(target.saturating_duration_since(Instant::now()));
    }
    if !changed {
        return Err(bad("counter did not advance"));
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct TraceRow {
    ns: u64,
    rgb: [u8; 3],
}
struct Trace {
    file: File,
    pending: Vec<u8>,
    last: Option<TraceRow>,
}
impl Trace {
    fn rows(&mut self) -> Result<Vec<TraceRow>> {
        self.file.read_to_end(&mut self.pending)?;
        let mut rows = Vec::new();
        let mut consumed = 0;
        for line in self.pending.split_inclusive(|&c| c == b'\n') {
            if line.last() != Some(&b'\n') {
                break;
            }
            consumed += line.len();
            let text = std::str::from_utf8(line)?.trim();
            if text.is_empty() {
                continue;
            }
            let cells: Vec<_> = text.split(',').collect();
            if cells.len() != 4 {
                return Err(bad("trace requires monotonic_ns,r,g,b"));
            }
            let row = TraceRow {
                ns: cells[0].parse()?,
                rgb: [cells[1].parse()?, cells[2].parse()?, cells[3].parse()?],
            };
            if self.last.is_some_and(|last| row.ns < last.ns) {
                return Err(bad("trace timestamps moved backward"));
            }
            self.last = Some(row);
            rows.push(row);
        }
        self.pending.drain(..consumed);
        Ok(rows)
    }
}
fn event(f: &mut File, kind: u16, code: u16, value: i32) -> Result<()> {
    let mut b = [0u8; 24];
    b[16..18].copy_from_slice(&kind.to_ne_bytes());
    b[18..20].copy_from_slice(&code.to_ne_bytes());
    b[20..24].copy_from_slice(&value.to_ne_bytes());
    f.write_all(&b)?;
    Ok(())
}
fn key(f: &mut File, down: bool) -> Result<()> {
    event(f, 1, 304, i32::from(down))?;
    event(f, 0, 0, 0)
}
fn response(path: &str, count: usize, trace_path: &str) -> Result<()> {
    if count == 0 {
        return Err(bad("count must be positive"));
    }
    let mut input = OpenOptions::new().write(true).open(path)?;
    let mut trace = Trace {
        file: File::open(trace_path)?,
        pending: Vec::new(),
        last: None,
    };
    key(&mut input, false)?;
    sleep(Duration::from_millis(250));
    println!("trial,pressed,latency_ms,before_r,before_g,before_b,after_r,after_g,after_b,injection_ns,swap_ns");
    let mut random = monotonic_ns()? ^ u64::from(std::process::id());
    let mut failures = 0;
    let result = (|| -> Result<()> {
        for trial in 0..count {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            sleep(Duration::from_micros(80_000 + random % 100_003));
            trace.rows()?;
            let before = trace
                .last
                .ok_or("trace has no initial controlled-ROM image")?;
            let down = trial % 2 == 0;
            let began = Instant::now();
            let injected = monotonic_ns()?;
            key(&mut input, down)?;
            loop {
                let response = trace.rows()?.into_iter().find(|row| {
                    row.ns >= injected
                        && row.ns - injected <= 250_000_000
                        && row.rgb[2] < 16
                        && if down {
                            row.rgb[1] > before.rgb[1]
                        } else {
                            row.rgb[0] > before.rgb[0]
                        }
                });
                let [r, g, b] = before.rgb;
                if let Some(row) = response {
                    let [ar, ag, ab] = row.rgb;
                    let ms = (row.ns - injected) as f64 / 1_000_000.;
                    println!(
                        "{trial},{down},{ms:.3},{r},{g},{b},{ar},{ag},{ab},{injected},{}",
                        row.ns
                    );
                    break;
                }
                if began.elapsed() >= Duration::from_millis(250) {
                    failures += 1;
                    println!("{trial},{down},NaN,{r},{g},{b},NaN,NaN,NaN,{injected},NaN");
                    break;
                }
                sleep(Duration::from_micros(500));
            }
        }
        Ok(())
    })();
    let release = key(&mut input, false);
    result?;
    release?;
    eprintln!(
        "software injection-to-changed-game-image swap return only; {failures}/{count} failures"
    );
    if failures > 0 {
        return Err(bad("response trials timed out"));
    }
    Ok(())
}
fn run() -> Result<()> {
    let a: Vec<_> = env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("sample") if a.len() == 5 => sample(&a[2], &a[3], a[4].parse()?),
        Some("response") if a.len() == 5 => response(&a[2], a[3].parse()?, &a[4]),
        _ => Err(bad(
            "usage: probe sample PID gpsp|gba|gb SECONDS | probe response EVENT COUNT TRACEFILE",
        )),
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("probe: {e}");
        std::process::exit(1);
    }
}
