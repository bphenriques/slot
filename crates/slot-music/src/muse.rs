//! plorpOS's Muse, one of the players slot can play through: its socket and the lines it
//! speaks, as `slot_player`'s commands and events.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

pub use slot_player::{Command, Event, Handle, PlayState};

/// Where plorpOS's Muse listens; it takes one client, the newest.
pub const SOCKET: &str = "/tmp/muse.sock";
const PROTO: &str = "2";
const RETRY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);

/// `command` as Muse reads it: a verb, then tab-separated key=value fields; none for what Muse
/// cannot do.
pub fn wire(command: &Command) -> Option<String> {
    Some(match command {
        Command::Play { path, at, speed } => {
            let mut line = format!("PLAY\tpath={}", path.display());
            if *at > 0.0 {
                line += &format!("\tat={at:.1}");
            }
            if *speed != 1.0 {
                line += &format!("\tspeed={speed:.2}");
            }
            line
        }
        Command::Pause => "PAUSE".into(),
        Command::Resume => "RESUME".into(),
        Command::Stop => "STOP".into(),
        Command::Seek(at) => format!("SEEK\tat={at:.1}"),
        Command::Speed(x) => format!("SPEED\tx={x:.2}"),
        Command::Mono(_)
        | Command::Level(_)
        | Command::Next(_)
        | Command::Crossfade(_)
        | Command::SkipSilence(_)
        | Command::Boost(_) => return None,
    })
}

/// A line of Muse's protocol: a verb, then tab-separated key=value fields.
pub fn parse(line: &str) -> Option<Event> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut fields = line.split('\t');
    let verb = fields.next()?;
    let get = |key: &str| {
        line.split('\t')
            .skip(1)
            .find_map(|f| f.strip_prefix(key)?.strip_prefix('='))
            .map(str::to_string)
    };
    match verb {
        "READY" => match get("proto").as_deref() {
            Some(PROTO) => None,
            other => Some(Event::Error(format!("unknown protocol {other:?}"))),
        },
        "STATE" => Some(Event::State(match get("state")?.as_str() {
            "playing" => PlayState::Playing,
            "paused" => PlayState::Paused,
            _ => PlayState::Stopped,
        })),
        "POS" => Some(Event::Pos {
            at: get("at")?.parse().ok()?,
            len: get("len")?.parse().ok()?,
        }),
        "END" => Some(Event::End(get("path")?.into())),
        "ERROR" => Some(Event::Error(get("why").unwrap_or_default())),
        "META" => Some(Event::Meta),
        "CHAPTER" => Some(Event::Chapter {
            at: get("at")?.parse().ok()?,
            title: get("title").unwrap_or_default(),
        }),
        _ => None,
    }
}

/// Muse at `socket`, through a connection that survives Muse restarting. Commands sent while it
/// is down are dropped, so nothing stale plays later.
pub fn muse(socket: PathBuf) -> Handle {
    Handle::spawn("slot-muse", move |commands, events| {
        run(&socket, &commands, &events)
    })
}

fn run(socket: &Path, commands: &Receiver<Command>, events: &Sender<Event>) {
    let mut said = false;
    loop {
        let stream = match UnixStream::connect(socket) {
            Ok(s) => s,
            Err(e) => {
                if !said {
                    eprintln!("slot: music: {}: {e}, retrying", socket.display());
                    said = true;
                }
                if commands.try_iter().count() > 0 {
                    let _ = events.send(Event::State(PlayState::Stopped));
                }
                std::thread::sleep(RETRY);
                continue;
            }
        };
        said = false;
        if events.send(Event::Connected(true)).is_err() || !serve(stream, commands, events) {
            return;
        }
        let _ = events.send(Event::Connected(false));
    }
}

/// Until Muse goes away (true) or slot drops its end (false).
fn serve(stream: UnixStream, commands: &Receiver<Command>, events: &Sender<Event>) -> bool {
    let Ok(reader) = stream.try_clone() else {
        return true;
    };
    if stream.set_read_timeout(Some(POLL)).is_err() {
        return true;
    }
    let mut reader = BufReader::new(reader);
    let mut writer = stream;
    let mut line = String::new();
    loop {
        loop {
            match commands.try_recv() {
                Ok(c) => {
                    let Some(line) = wire(&c) else {
                        continue;
                    };
                    if writeln!(writer, "{line}").is_err() {
                        return true;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
        match reader.read_line(&mut line) {
            Ok(0) => return true,
            Ok(_) if line.ends_with('\n') => {
                if let Some(event) = parse(&line) {
                    if events.send(event).is_err() {
                        return false;
                    }
                }
                line.clear();
            }
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return true,
        }
    }
}
