//! The performance log, from the Debug page: while it is on, frame timings are kept, and a
//! thread of its own writes a line every few seconds to `Logs/music-perf.log` on the card, with
//! each thread's share of the processor from `/proc`, to say where the time goes on the device.
//! Off, nothing is kept and no thread runs.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
}

/// The log, running: its thread stops when this is dropped.
pub(crate) struct PerfLog {
    pub stats: Arc<Stats>,
    stop: Arc<AtomicBool>,
}

impl PerfLog {
    pub fn start(root: &Path) -> PerfLog {
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (kept, stopped, path) = (stats.clone(), stop.clone(), root.join(PERF_FILE));
        let spawned = std::thread::Builder::new()
            .name("slot-music-perf".into())
            .spawn(move || write(&path, &kept, &stopped));
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        PerfLog { stats, stop }
    }
}

impl Drop for PerfLog {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Every `EVERY`, a line: the timings, each thread's share of one core, and the memory held.
fn write(path: &PathBuf, stats: &Stats, stop: &AtomicBool) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut was = cpu();
    let mut since = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(250));
        if since.elapsed() < EVERY {
            continue;
        }
        let secs = since.elapsed().as_secs_f64();
        let now = cpu();
        let line = line(stats, &was, &now, secs);
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

fn line(stats: &Stats, was: &Cpu, now: &Cpu, secs: f64) -> String {
    let (poll, poll_max, frames) = stats.poll.take();
    let (screen, screen_max, _) = stats.screen.take();
    let (glue, glue_max, _) = stats.glue.take();
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
            format!("{name}={share:.1}%")
        })
        .collect();
    format!(
        "{} {secs:.0}s {what} | {:.0} fps | music {poll}us (max {poll_max}) screen {screen}us \
         (max {screen_max}) draw {glue}us (max {glue_max}) | cpu {} | rss {} MB",
        slot_store::stamp_now(),
        frames as f64 / secs,
        threads.join(" "),
        now.rss_mb,
    )
}

/// Each thread's processor time so far, by its name, in clock ticks; and the memory held.
#[derive(Default)]
struct Cpu {
    threads: BTreeMap<String, u64>,
    per_sec: f64,
    rss_mb: u64,
}

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
    Cpu {
        threads,
        per_sec,
        rss_mb: pages * 4096 / (1 << 20),
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
    fn a_thread_s_name_and_time_are_read_from_its_stat_line() {
        let stat = "123 (slot-player o) S 1 2 3 4 5 6 7 8 9 10 40 2 0 0 20 0 9 0 99";
        assert_eq!(thread_ticks(stat), Some(("slot-player o".into(), 42)));
        assert_eq!(thread_ticks("garbage"), None);
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
