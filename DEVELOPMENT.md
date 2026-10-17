# Developing this fork

This fork is upstream slot (pvaibhav/slot, itself BrandonKowalski/slot) plus a music and
audiobook player. Keep it that way: new behaviour lives in new crates and modules, upstream
files change only at a few one-line hooks, so `git merge upstream/...` stays cheap.

## Layout

| Where | What |
|---|---|
| `crates/slot-music` | The player's behaviour, no drawing: library scan and cache, queue, screens as data, keys, settings, audiobooks (`books/`). Tested without a device. |
| `crates/slot-player` | Plays files: the `Command`/`Event` words and `Handle`, the face every player has. slot's own engine (rodio decode, rubato resample, SoundTouch speed, ReplayGain, mono, gapless, ALSA). |
| `crates/slot-player/soundtouch` | SoundTouch 2.3.2 sources (LGPL-2.1), built by `build.rs` with `cc`. |
| `crates/slot-music/src/muse.rs` | plorpOS's Muse as one more player behind `Handle`, over its socket. |
| `crates/slot-ui/src/music` | Drawing: faces, lists, Now Playing, `theme.rs` (every colour). |
| `crates/slot/src/music_faces`, `crates/slot/src/app/music.rs` | The frontend glue: textures, animation (`motion.rs`), input routing. |
| `music/` | `CONTROLS.md` (every button, keep it current), `music.txt.example`, `fetch-muse.sh`. |
| `card/System/muse.sh` | Starts Muse at boot, unless `player = slot`. |

Upstream files touched: `crates/slot/src/app.rs`, `session.rs`, `crates/slot-ui/src/lib.rs`,
`quick_menu.rs`, `taskfile.yml`, `README.md`, `RELEASE_NOTES.md`. Keep those diffs small.

## Set up

- NixOS/Nix: `nix-shell` gives `task`, `rustup`, rsync and the rest (`shell.nix`); the toolchain
  is pinned in `rust-toolchain.toml`. Run cargo through it: `nix-shell --run "cargo ..."`.
- Device builds run in an arm64 `rust:1-bullseye` container (Docker, plus aarch64 binfmt
  emulation on an x86_64 host; see the top of `shell.nix`). It has g++, which SoundTouch needs;
  it has no libclang or ALSA headers, which is why nothing here uses bindgen or links ALSA.

## Build, test, lint

```sh
nix-shell --run "cargo test -p slot-music -p slot-ui -p slot-input -p slot-player"
nix-shell --run "cargo test -p slot --no-default-features --features slot/device --lib"
nix-shell --run "cargo clippy -p slot -p slot-services --no-default-features --features slot/device -- -D warnings"
nix-shell --run "cargo clippy -p slot-music -p slot-ui -p slot-input -p slot-player --tests -- -D warnings"
nix-shell --run "cargo fmt -p slot-music -p slot-ui -p slot -p slot-player"
git checkout -- crates/slot/tests/prefetch.rs crates/slot/tests/runahead.rs   # fmt touches these upstream files; keep them as upstream has them
```

All of it passes before anything goes on a card. On an x86 laptop, slot's own `input`, `bios`,
`core`, `core_options` and emulator timing tests fail regardless of changes; the music lib tests
above do not. `every_music_legend_fits_across_the_screen` (slot lib) fails when a hint is too
long.

## Deploy

```sh
nix-shell --run "task dist:device"                        # builds dist-device/
C=/run/media/$USER/<card>                                  # SD2, the frontend card
mount | grep "$C"; cat /sys/block/mmcblk0/ro               # must be rw and 0 (the lock switch)
nix-shell --run "task sync:card CARD=$C"                   # adds and replaces; keeps games, saves, config
cmp dist-device/System/slot "$C/System/slot" && sync       # verify, then unmount
```

Over adb instead: `task deploy:device`. Logs: `Logs/slot.log` on the card (or
`task log:device`); Muse logs to `muse.log`. Player settings live in `Config/music.txt`
(`player = slot` for slot's own player, read at start).

## Conventions

- Simple, idiomatic, succinct, composable; YAGNI; familiar UX (look at how common players do it).
- Every user-visible word in `crates/slot-music/src/text.rs`; every colour and level in
  `crates/slot-ui/src/music/theme.rs`. No literals elsewhere.
- Logic emits values (`Outcome`, `Notice`, `Legend`); `slot-ui` and the frontend only draw.
- Buttons: a tap does one thing, a hold another, shown as "Tap (Held)"; hints never change
  while held (the corner ring shows the hold). START menus hold only what no button does.
  Update `music/CONTROLS.md` with any key change.
- Comments and docs in the existing voice: plain sentences saying what something is for.
- Tests for behaviour (`crates/slot-music/tests/music.rs`, `crates/slot-player/tests/player.rs`,
  which plays WAVs made in the test into a `Capture` output). Timing tests keep wide margins.
- Performance: the device is a battery handheld (H700, Cortex-A53). Nothing polls while idle;
  work only while playing; measure before and after (see Status).
- Editing with scripts: no Python on the host; with perl, `\|` in a `|`-delimited regex is an
  alternation (use `~`), `'` comes out literally from heredoc variables (fix with
  `sed -i "s/\\\'/'/g"`), and check a temp file with `test -s` before copying it over.

## Git

- Commit only features the owner has tested end to end on the device; never per change.
- Author email `4727729+bphenriques@users.noreply.github.com`; no other address ever.
- Never push or tag without an explicit go. The version is `v1.7.0-bphenriques.1`.
- `stash@{0}` is old music work from before the v1.7.0 merge; drop it once happy.

## Status and what is left

Everything below the last commit (`cb2326e`) is uncommitted: the music overhaul, audiobooks,
`slot-player`. Next steps, in order:

1. On the device with `player = slot`: speaker, Bluetooth headphones, a game while paused (the
   device hand-over: BaseOS has no mixer, one opener at a time), a gapless album, Volume
   Leveling, Mono, a book at 1.5x, seeking in an `.m4b`, a damaged file.
2. Measure CPU on the device against Muse (`top` while playing; Muse was ~1% of a core). On
   the laptop: decode 1090x real time, decode + resample 669x, at 1.5x 432x.
3. Then commit (per the rule above), and decide whether Muse goes: `player = slot` as default,
   then drop `vendor/muse`, `task muse`, `muse.sh` and the FFmpeg source duty.
4. Release notes and licences: SoundTouch's is in `licenses/`; list rodio, symphonia, rubato
   and lofty with the other third-party crates.

Known gaps: chapters of non-MP4 books show only once played; Bluetooth output is whatever ALSA's
`default` is (no device choice yet); the old keys in the card's `music.txt` (`skip`, `rows`,
`fade`, `animations`) are ignored; its `mono = on`, from an earlier try, takes effect with
`player = slot`. Playlist editing is out of scope (see
`music/CONTROLS.md`).

Ideas, if wanted: Bluetooth output choice; equalizer presets; crossfade only between albums. Each fits as a stage in
`slot-player/src/track.rs` or `engine.rs`, behind a setting shown with slot's player only.
