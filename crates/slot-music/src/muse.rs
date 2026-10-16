use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

/// Where plorpOS's Muse listens; it takes one client, the newest.
pub const SOCKET: &str = "/tmp/muse.sock";
const PROTO: &str = "2";
const RETRY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum MuseState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// What slot asks of Muse, which only plays files; the queue is slot's.
#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    /// Open and play `path`, from `at` seconds in, at `speed` (1 is as recorded).
    Play {
        path: PathBuf,
        at: f64,
        speed: f32,
    },
    Pause,
    Resume,
    Stop,
    Seek(f64),
    /// How fast to play, 0.5 to 2, the voice kept at its pitch.
    Speed(f32),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Play { path, at, speed } => {
                write!(f, "PLAY\tpath={}", path.display())?;
                if *at > 0.0 {
                    write!(f, "\tat={at:.1}")?;
                }
                match *speed == 1.0 {
                    true => Ok(()),
                    false => write!(f, "\tspeed={speed:.2}"),
                }
            }
            Command::Pause => f.write_str("PAUSE"),
            Command::Resume => f.write_str("RESUME"),
            Command::Stop => f.write_str("STOP"),
            Command::Seek(at) => write!(f, "SEEK\tat={at:.1}"),
            Command::Speed(x) => write!(f, "SPEED\tx={x:.2}"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Connected(bool),
    State(MuseState),
    Pos {
        at: f64,
        len: f64,
    },
    End(PathBuf),
    Error(String),
    /// A file was opened: its chapters follow, each where it starts and its title.
    Meta,
    Chapter {
        at: f64,
        title: String,
    },
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
            "playing" => MuseState::Playing,
            "paused" => MuseState::Paused,
            _ => MuseState::Stopped,
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

/// A connection to Muse that survives Muse restarting. Commands sent while it is down are
/// dropped, so nothing stale plays later.
pub struct Muse {
    commands: Sender<Command>,
    events: Receiver<Event>,
}

impl Muse {
    pub fn spawn(socket: PathBuf) -> Muse {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-muse".into())
            .spawn(move || run(&socket, &command_rx, &event_tx));
        if let Err(e) = spawned {
            eprintln!("slot: music: {e}");
        }
        Muse { commands, events }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }
}

fn run(socket: &PathBuf, commands: &Receiver<Command>, events: &Sender<Event>) {
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
                    let _ = events.send(Event::State(MuseState::Stopped));
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
                    if writeln!(writer, "{c}").is_err() {
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
