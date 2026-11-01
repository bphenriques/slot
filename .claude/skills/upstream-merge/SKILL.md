---
name: upstream-merge
description: Merge a new pvaibhav/slot release into this fork and prepare the matching vX.Y.Z-bphenriques.N. Use when the owner says "update with upstream", "merge upstream", or names a new pvaibhav or slot version.
---

# Merge upstream

1. **See what is new.** `git fetch upstream --tags`, then `git log --oneline --first-parent
   HEAD..upstream/pvaibhav-improvements` and upstream's `CHANGELOG.md` and `RELEASE_NOTES.md`.
2. **Merge** on `bphenriques-improvements`: `git merge --no-commit --no-ff
   upstream/pvaibhav-improvements`. Conflicts recur:
   - `crates/slot-ui/src/quick_menu.rs`: keep both sides' rows; `ALL: [QuickRow; N]` is theirs plus ours.
   - `README.md`, `RELEASE_NOTES.md`, `DEVELOPMENT.md`: ours.
   - `.github/workflows/release.yml`: ours, plus any new upstream step or permission.
   - Any other upstream file is one of our hooks: keep it one line.
3. **Check.** `cargo fmt -p slot -p slot-ui`, `git checkout -- crates/slot/tests/prefetch.rs`,
   `task check:music`, then `task dist:device`. The footprint must not grow past the last merge's.
4. **Version.** `vX.Y.Z-bphenriques.1` for the first on slot X.Y.Z, then `.2`. Update the heading,
   base and what's new (from slot, from pvaibhav) in `RELEASE_NOTES.md`, the release link in
   `README.md`, and the version line in `DEVELOPMENT.md`.
5. **Report** conflicts, footprint, and anything new worth trying or scoping. Stop there: releasing
   is the `release` skill, on the owner's go.

## Gotchas

- `shell.nix` broken: `nix-shell -p go-task rustup cmake jq zig cargo-zigbuild rsync file curl unzip`.
- Cargo can't fetch mute over SSH: `export CARGO_NET_GIT_FETCH_WITH_CLI=true`.
- A `--locked` error as `check:music` starts, after moving `rev`, is the taskfile reading the old
  lock; the run then updates it, and later tasks are fine.
- mGBA "dubious ownership": `rm -rf target-device/mgba` and rebuild.
