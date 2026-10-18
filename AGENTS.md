# Agents

This is a fork of slot (a handheld frontend) with a music and audiobook player added. Read
`DEVELOPMENT.md` for setup, deploying and the state of things; these are the rules.

## Shape

Layers, each knowing only the one below it:

| Crate | Owns | Never |
|---|---|---|
| `slot-player` | Playing files: decode, resample, speed, gain, gapless, ALSA. `Command` in; its `Status` (state, place, length) read each frame; `Event` out only for what happens (an end, going on into the next, a failure, the device lost, the player gone); behind `Handle`. | Albums, books, queues, screens. |
| `slot-music` | The player's behaviour: library, queue, books, lyrics, keys, settings, and every screen as data (`Screen`). Every word in `text.rs`; every file it keeps on the card in `store/`. | Drawing, textures, colours. |
| `slot-ui::music` | The kit that draws a `Screen`'s parts: faces, list, Now Playing, text view, `tween`, every colour in `theme.rs`. | Behaviour, state. |
| `slot::music_faces`, `slot::app::music` | Glue: a `Screen` into the kit's faces each frame; keys into `slot-music`. | Decisions the layers above make. |

`Screen` is the only contract between behaviour and drawing. New behaviour goes in `slot-music`
with tests; new looks go in the kit. Upstream slot's own files change only at one-line hooks.

## Rules

- Simple, idiomatic, succinct; YAGNI. Do what familiar players do (Apple Music, Spotify,
  Symfonium, Smart AudioBook Player, Rockbox).
- `Music` (`slot-music/src/music/`) is state and a shell around it. What happens comes in as
  messages (the player's events, the scan, the worker) or keys; the state acts and asks for
  what it needs outside as an `Effect` (tell the player, write a file, set a key, give the
  worker a job), which the shell carries out after. The state never touches the card or a
  thread, and never waits: a slow read (pictures, lyrics, chapters) is a worker job.
- State is plain values; only `slot-music/src/store/` reads and writes what is kept on the card,
  each file a `Kept` type (`encode`/`decode`, tested on text) written off the frame loop by the
  one `Store`. The library scan, its cache and the album pictures keep their own files.
- No literal words outside `slot-music/src/text.rs`, no literal colours outside
  `slot-ui/src/music/theme.rs`, no motion outside `slot-ui/src/music/tween.rs`'s curve and
  durations.
- Keys: a tap does one thing, a hold another, hinted "Tap (Held)"; a START menu holds only what
  no key does. Update `music/CONTROLS.md` with any key change.
- Comments read as plain sentences saying what something is for, in the existing voice.
- Every behaviour change comes with a test: `slot-music/tests/music/` (a module an area),
  `slot-player/tests/player.rs` (WAVs made in the test, played into `Capture`), or unit tests
  beside the code.
- A battery device: nothing polls while idle, nothing works harder than it must.

## Check before you finish

```sh
nix-shell --run "cargo test -p slot-music -p slot-ui -p slot-input -p slot-player"
nix-shell --run "cargo test -p slot --no-default-features --features slot/device --lib"
nix-shell --run "cargo clippy -p slot -p slot-services --no-default-features --features slot/device -- -D warnings"
nix-shell --run "cargo clippy -p slot-music -p slot-ui -p slot-input -p slot-player --tests -- -D warnings"
nix-shell --run "cargo fmt -p slot-music -p slot-ui -p slot -p slot-player"
git checkout -- crates/slot/tests/prefetch.rs crates/slot/tests/runahead.rs
```

A device build (`task dist:device`) takes minutes; only when asked. Never push, tag or commit
without the owner's go; commits only for features tested on the device, authored as
`4727729+bphenriques@users.noreply.github.com`, no other address.

## Editing

Back a file up before splicing it by line numbers or patterns; check the result is not empty
before copying it over. There is no Python on the host. With perl, use `~` as the delimiter, and
note that `\x27` comes out literally from shell heredocs.
