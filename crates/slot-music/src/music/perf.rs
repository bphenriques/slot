//! The performance log, from the Debug page: while it is on, frame timings are kept, and a
//! thread of its own writes a line every few seconds to `Logs/music-perf.log` on the card, with
//! each thread's share of the processor from `/proc`, to say where the time goes on the device.
//! Off, nothing is kept and no thread runs.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Where the log goes, from the card's root, and how often a line is written.
pub(crate) const PERF_FILE: &str = "Logs/music-perf.log";
const EVERY: Duration = Duration::from_secs(10);

/// One kind of work timed: how many times, how long in all, and the longest, in nanoseconds.
#[derive(Default)]
pub(crate) struct Timing {
    n: AtomicU64,
    sum: AtomicU64,
    max: AtomicU64,
}

impl Timing {
    pub fn add(&self, took: Duration) {
        let ns = took.as_nanos() as u64;
        self.n.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(ns, Ordering::Relaxed);
        self.max.fetch_max(ns, Ordering::Relaxed);
    }

    /// The average and the longest, in microseconds, since last asked; and how many.
    fn take(&self) -> (u64, u64, u64) {
        let n = self.n.swap(0, Ordering::Relaxed);
        let sum = self.sum.swap(0, Ordering::Relaxed);
        let max = self.max.swap(0, Ordering::Relaxed);
        (sum / n.max(1) / 1000, max / 1000, n)
    }
}

/// What the frame loop keeps while the log is on: the music's own frame, building its screen,
/// and the frontend drawing it; and what was shown, said in a few words.
#[derive(Default)]
pub(crate) struct Stats {
    pub poll: Timing,
    pub screen: Timing,
    pub glue: Timing,
    pub what: Mutex<String>,
    /// Files the player was told to open: a song played, skipped to or gone on into.
    pub opened: AtomicU64,
    /// The player's work as it last said, to tell where its share of a core went.
    pub player: Mutex<slot_player::Work>,
}

/// The log, running: its thread stops when this is dropped, the channel it waits on closing.
pub(crate) struct PerfLog {
    pub stats: Arc<Stats>,
    _running: Sender<()>,
}

impl PerfLog {
    pub fn start(root: &Path) -> PerfLog {
        let stats = Arc::new(Stats::default());
        let (running, stopped) = mpsc::channel();
        let (kept, path) = (stats.clone(), root.join(PERF_FILE));
        let spawned = std::thread::Builder::new()
            .name("slot-music-perf".into())
            .spawn(move || write(&path, &kept, &stopped));
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        PerfLog {
            stats,
            _running: running,
        }
    }
}

/// Every `EVERY`, a line: the timings, each thread's share of one core, and the memory held.
fn write(path: &PathBuf, stats: &Stats, stopped: &Receiver<()>) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut was = cpu();
    let mut worked = slot_player::Work::default();
    let mut since = Instant::now();
    // Asleep the while between lines; woken early only to stop.
    while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(EVERY) {
        let secs = since.elapsed().as_secs_f64();
        let now = cpu();
        let work = *stats.player.lock().unwrap_or_else(PoisonError::into_inner);
        let line = line(stats, (&was, &now), (worked, work), secs);
        worked = work;
        (was, since) = (now, Instant::now());
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
        if let Err(e) = file.and_then(|mut f| writeln!(f, "{line}")) {
            eprintln!("slot: music: {}: {e}", path.display());
        }
    }
}

/// What the player did between `worked` and `work`, `secs` apart, each as a share of one core:
/// of its writing, how much was in ALSA's library.
fn shares(worked: slot_player::Work, work: slot_player::Work, secs: f64) -> String {
    let share = |was: u64, is: u64| is.saturating_sub(was) as f64 / (secs * 1e9) * 100.0;
    format!(
        "decode {:.2}% mix {:.2}% write {:.2}% (alsa {:.2}%)",
        share(worked.decode, work.decode),
        share(worked.mix, work.mix),
        share(worked.write, work.write),
        share(worked.library, work.library),
    )
}

/// A line of the log: the timings, what the player did since `worked` (its decoding, mixing and
/// writing, each a share of one core), each thread's share, and the memory held.
fn line(
    stats: &Stats,
    (was, now): (&Cpu, &Cpu),
    (worked, work): (slot_player::Work, slot_player::Work),
    secs: f64,
) -> String {
    let player = shares(worked, work, secs);
    let (poll, poll_max, frames) = stats.poll.take();
    let (screen, screen_max, _) = stats.screen.take();
    // The faces are built only for a frame drawn: their count is the frames drawn.
    let (glue, glue_max, drawn) = stats.glue.take();
    let opened = stats.opened.swap(0, Ordering::Relaxed);
    let what = stats
        .what
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let threads: Vec<String> = now
        .threads
        .iter()
        .map(|(name, &ticks)| {
            let used = ticks.saturating_sub(was.threads.get(name).copied().unwrap_or(0));
            let share = used as f64 / now.per_sec / secs * 100.0;
            format!("{name}={share:.2}%")
        })
        .collect();
    format!(
        "{} {secs:.0}s {what} | {:.0} fps, {:.0} drawn | music {poll}us (max {poll_max}) screen {screen}us \
         (max {screen_max}) draw {glue}us (max {glue_max}) | opened {opened} | sound {} | player {player} | clock {} | cpu {} \
         | rss {} MB",
        slot_store::stamp_now(),
        frames as f64 / secs,
        drawn as f64 / secs,
        sound(),
        clock(was, now),
        threads.join(" "),
        now.rss_mb,
    )
}

/// What each playback device open is really running at, as the kernel says: its format, rate
/// and channels, past any conversion ALSA's library does on the way; "closed" when none is.
fn sound() -> String {
    let cards = std::fs::read_dir("/proc/asound")
        .into_iter()
        .flatten()
        .flatten();
    let pcms = cards.flat_map(|card| {
        std::fs::read_dir(card.path())
            .into_iter()
            .flatten()
            .flatten()
    });
    let subs = pcms
        .filter(|p| p.file_name().to_string_lossy().ends_with('p'))
        .flat_map(|p| std::fs::read_dir(p.path()).into_iter().flatten().flatten());
    let open: Vec<String> = subs
        .filter_map(|s| hw_params(&std::fs::read_to_string(s.path().join("hw_params")).ok()?))
        .collect();
    match open.is_empty() {
        true => "closed".into(),
        false => open.join(", "),
    }
}

/// A device's `hw_params` said short: "S16_LE 44100 Hz 2 ch"; none while it is closed.
fn hw_params(text: &str) -> Option<String> {
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .and_then(|v| v.split_whitespace().next())
    };
    Some(format!(
        "{} {} Hz {} ch",
        field("format")?,
        field("rate")?,
        field("channels")?
    ))
}

/// Each thread's processor time so far, by its name, in clock ticks; the memory held; and the
/// processor's clock: how long it has run at each speed, where the kernel keeps that, the speed
/// now, and the governor choosing it, as a share of a core means more work at a slower clock.
#[derive(Default)]
struct Cpu {
    threads: BTreeMap<String, u64>,
    per_sec: f64,
    rss_mb: u64,
    speeds: BTreeMap<u64, u64>,
    khz: Option<u64>,
    governor: String,
}

/// Where the first core's clock is told: the four of the H700 share it.
const CPUFREQ: &str = "/sys/devices/system/cpu/cpu0/cpufreq";

fn cpu() -> Cpu {
    let mut threads = BTreeMap::new();
    let tasks = std::fs::read_dir("/proc/self/task").into_iter().flatten();
    for task in tasks.flatten() {
        let stat = std::fs::read_to_string(task.path().join("stat")).unwrap_or_default();
        if let Some((name, ticks)) = thread_ticks(&stat) {
            *threads.entry(name).or_insert(0) += ticks;
        }
    }
    // SAFETY: sysconf only reads a constant of the system.
    let per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
    let pages: u64 = std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0);
    let read =
        |name: &str| std::fs::read_to_string(Path::new(CPUFREQ).join(name)).unwrap_or_default();
    Cpu {
        threads,
        per_sec,
        rss_mb: pages * 4096 / (1 << 20),
        speeds: time_in_state(&read("stats/time_in_state")),
        khz: read("scaling_cur_freq").trim().parse().ok(),
        governor: read("scaling_governor").trim().to_string(),
    }
}

/// How long the clock has run at each speed, in kHz, from `time_in_state`: a speed and a time
/// a line.
fn time_in_state(text: &str) -> BTreeMap<u64, u64> {
    let pair = |line: &str| {
        let mut fields = line.split_whitespace().map(|f| f.parse::<u64>().ok());
        Some((fields.next()??, fields.next()??))
    };
    text.lines().filter_map(pair).collect()
}

/// The clock between `was` and `now`, in MHz: its average, where the kernel keeps how long it
/// ran at each speed, else the speed now; and the governor choosing it.
fn clock(was: &Cpu, now: &Cpu) -> String {
    let ran = |khz: &u64, t: &u64| t.saturating_sub(was.speeds.get(khz).copied().unwrap_or(0));
    let total: u64 = now.speeds.iter().map(|(k, t)| ran(k, t)).sum();
    let weighted: u64 = now.speeds.iter().map(|(k, t)| k * ran(k, t)).sum();
    let speed = match (total, now.khz) {
        (1.., _) => format!("{} MHz average", weighted / total / 1000),
        (0, Some(khz)) => format!("{} MHz now", khz / 1000),
        (0, None) => "unknown".into(),
    };
    match now.governor.as_str() {
        "" => speed,
        governor => format!("{speed}, {governor}"),
    }
}

/// A thread's name and its user and system time, from its `stat` line: the name in brackets,
/// which may hold spaces, and the times the 12th and 13th fields after it.
fn thread_ticks(stat: &str) -> Option<(String, u64)> {
    let (open, close) = (stat.find('(')?, stat.rfind(')')?);
    let name = stat.get(open + 1..close)?.to_string();
    let mut fields = stat.get(close + 2..)?.split_whitespace();
    let user: u64 = fields.nth(11)?.parse().ok()?;
    let system: u64 = fields.next()?.parse().ok()?;
    Some((name, user + system))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_s_work_is_told_as_shares_of_a_core_since_the_last_line() {
        let work = |decode, mix, write, library| slot_player::Work {
            decode,
            mix,
            write,
            library,
        };
        let told = shares(
            work(1_000_000_000, 0, 0, 0),
            work(1_400_000_000, 10_000_000, 2_000_000_000, 600_000_000),
            10.0,
        );
        assert_eq!(told, "decode 4.00% mix 0.10% write 20.00% (alsa 6.00%)");
        assert_eq!(
            shares(work(5, 5, 5, 5), work(0, 0, 0, 0), 10.0),
            "decode 0.00% mix 0.00% write 0.00% (alsa 0.00%)",
            "a player begun again counts from nothing"
        );
    }

    #[test]
    fn a_thread_s_name_and_time_are_read_from_its_stat_line() {
        let stat = "123 (slot-player o) S 1 2 3 4 5 6 7 8 9 10 40 2 0 0 20 0 9 0 99";
        assert_eq!(thread_ticks(stat), Some(("slot-player o".into(), 42)));
        assert_eq!(thread_ticks("garbage"), None);
    }

    #[test]
    fn a_device_s_format_and_rate_are_read_from_its_hw_params() {
        let open = "access: RW_INTERLEAVED\nformat: S16_LE\nsubformat: STD\nchannels: 2\nrate: 44100 (44100/1)\nperiod_size: 1024\n";
        assert_eq!(hw_params(open).as_deref(), Some("S16_LE 44100 Hz 2 ch"));
        assert_eq!(hw_params("closed\n"), None);
    }

    #[test]
    fn the_clock_is_its_average_since_the_last_line_or_else_its_speed_now() {
        let at = |text: &str, khz, governor: &str| Cpu {
            speeds: time_in_state(text),
            khz,
            governor: governor.into(),
            ..Cpu::default()
        };
        let was = at("480000 100\n1008000 50\n", None, "ondemand");
        // 3 s at 480 MHz and 1 s at 1008 since: 612 on average.
        let now = at("480000 400\n1008000 150\n", Some(1_008_000), "ondemand");
        assert_eq!(clock(&was, &now), "612 MHz average, ondemand");
        let bare = at("", Some(816_000), "");
        assert_eq!(
            clock(&Cpu::default(), &bare),
            "816 MHz now",
            "where no times are kept"
        );
        assert_eq!(clock(&Cpu::default(), &Cpu::default()), "unknown");
    }

    #[test]
    fn timings_say_their_average_and_longest_and_start_again() {
        let t = Timing::default();
        t.add(Duration::from_micros(100));
        t.add(Duration::from_micros(300));
        assert_eq!(t.take(), (200, 300, 2));
        assert_eq!(t.take(), (0, 0, 0), "since last asked");
    }
}
