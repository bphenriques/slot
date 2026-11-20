---
name: release
description: Commit, push and tag a vX.Y.Z-bphenriques.N release of this fork, then confirm it published. Use only on the owner's explicit go ("push and tag", "release", "retag").
---

# Release

1. **Ready.** `task check:music` passes, the change was tested on the device, and the first line
   of `RELEASE_NOTES.md` is `# <tag>` (the workflow refuses otherwise).
2. **Commit** on `bphenriques-improvements`, author `4727729+bphenriques@users.noreply.github.com`
   only, then `git push origin bphenriques-improvements`.
3. **Tag.** `task tag:music VERSION=<tag>` (in `nix-shell -p go-task rustup cmake jq zig
   cargo-zigbuild rsync file curl unzip bun pkg-config alsa-lib udev cargo-nextest git`, with
   `CARGO_NET_GIT_FETCH_WITH_CLI=true`), then `git push origin <tag>`. It runs `check:music`, then
   lint, lint:device, lint:nds, the suite minus the eleven vendored-core tests (they need a core an
   x86_64 host can load; upstream ran them on its own host, on this release) and test:card on HEAD
   in a clean worktree, and tags HEAD with the `Checked:` header the workflow demands. Takes about
   half an hour once the check cache is warm; it must see a clean tree. To retag after a fix:
   `git tag -d <tag> && git push origin :refs/tags/<tag>`, then tag again. The workflow
   republishes the release with the new card and notes.
4. **Confirm.** Local `gh` is not logged in, so use the public API:
   ```sh
   curl -s "https://api.github.com/repos/bphenriques/slot/actions/runs?per_page=3" \
     | jq '.workflow_runs[] | {name, head_branch, status, conclusion}'
   curl -s https://api.github.com/repos/bphenriques/slot/releases/tags/<tag> | jq '.assets[].name'
   ```
   Then download the zip and check it has `Cheats/Cheats.pack` (only a warning in the run if missing).

Deleting a release or draft needs `! gh auth login` or the web page.
