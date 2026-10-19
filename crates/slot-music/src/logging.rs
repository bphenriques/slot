//! The player's log, on slot's: each line `slot: <area>: <what>` on stderr, which slot's
//! launcher keeps as `Logs/slot.log`. What went wrong is always told; what happens as it plays
//! (the player's commands, fades, the buttons as read) only with Detailed Log on, so someone
//! asking for help turns it on and sends the file.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct Stderr;

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("slot: {}: {}", area(record.target()), record.args());
        }
    }

    fn flush(&self) {}
}

/// The area a line is from, by its crate: `slot_player::engine` is the player's.
fn area(target: &str) -> &str {
    let krate = target.split("::").next().unwrap_or(target);
    krate.strip_prefix("slot_").unwrap_or(krate)
}

/// The levels told: problems always; with `detailed`, what happens too.
fn level(detailed: bool) -> LevelFilter {
    if detailed {
        Level::Info.to_level_filter()
    } else {
        Level::Warn.to_level_filter()
    }
}

/// Logs from here on, `detailed` or not. Started once; later, only the level changes.
pub(crate) fn start(detailed: bool) {
    static STDERR: Stderr = Stderr;
    let _ = log::set_logger(&STDERR);
    log::set_max_level(level(detailed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_its_area_and_only_problems_are_told_unless_detailed() {
        assert_eq!(area("slot_player::engine"), "player");
        assert_eq!(area("slot_music::music::act"), "music");
        assert_eq!(area("slot::audio::line_out"), "slot");
        assert_eq!(level(false), LevelFilter::Warn);
        assert_eq!(level(true), LevelFilter::Info);
    }
}
