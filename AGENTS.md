# Agents

This is a fork of slot (a handheld frontend) with a music and audiobook player built in. The
player is [mute](https://github.com/bphenriques/mute-player), its own repository, which slot
takes as crates at a pinned revision; its rules, tests and screenshots live there, in its
`AGENTS.md`. Read `DEVELOPMENT.md` for setup, deploying and the state of things; these are the
rules here.

## Shape

| Where | Owns | Never |
|---|---|---|
| mute (`mute-audio`, `mute`, `mute-ui`, `mute-view`) | The player: playing files, its behaviour and every screen as data, the kit that draws them, and the view that makes a screen into the kit's faces. Changed in mute-player, then the revision here moved. | slot. |
| `slot::music_faces`, `slot::app::music` | slot as mute's host: the view's faces into its compositor's textures and draws; its buttons into `mute`; the screen off, power, volume and the lid. | Decisions mute makes. |

The glue is thin: what slot alone has (its app, its compositor, its keys, its lid) goes in, and
what mute gives (faces, sound, the rumble) goes out. Upstream slot's own files change only at
one-line hooks.

## Rules

- Simple, idiomatic, succinct; YAGNI. A change to the player is made in mute-player, with its
  tests and screenshots, and comes here by moving the revision.
- Rust as the community writes it, not a house style: rustfmt and clippy as they are, `if` over
  a `match` on a bool, and a `// SAFETY:` comment on every `unsafe` saying why it holds. The
  lints keep it so (`#![warn(...)]` atop our modules in slot's crates).
- Comments read as plain sentences saying what something is for, in the existing voice.
- Every behaviour change comes with a test.
- A battery device: nothing polls while idle, nothing works harder than it must. The music
  screen is drawn only when a frame would show something new (`App::holds_still`).

## Working on mute from here

To try a change to mute in slot before it is pushed, point Cargo at a checkout of it, in a
`.cargo/config.toml` kept out of git:

```toml
[patch."https://github.com/bphenriques/mute-player"]
mute = { path = "../mute-player/crates/mute" }
mute-audio = { path = "../mute-player/crates/mute-audio" }
mute-ui = { path = "../mute-player/crates/mute-ui" }
mute-view = { path = "../mute-player/crates/mute-view" }
```

Once it is in mute-player, move `rev` in `crates/slot/Cargo.toml` to its commit.

## Check before you finish

```sh
nix-shell --run "cargo fmt -p slot -p slot-ui"
nix-shell --run "task check:music"
```

`check:music` checks formatting, lints, runs slot's tests with its glue, and ends with
`footprint`: slot's own files as changed from upstream's. A footprint that grew is a hook too
many.

A device build (`task dist:device`) takes about a minute; only when asked. Never push, tag or commit
without the owner's go; commits only for features tested on the device, authored as
`4727729+bphenriques@users.noreply.github.com`, no other address.

## Editing

Back a file up before splicing it by line numbers or patterns; check the result is not empty
before copying it over. There is no Python on the host. With perl, use `~` as the delimiter, and
note that `\x27` comes out literally from shell heredocs.
