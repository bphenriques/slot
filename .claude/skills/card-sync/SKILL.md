---
name: card-sync
description: Copy dist-device/ onto the device's SD card. Use only when the owner's latest message says sync or that the card is in.
---

# Sync the card

Run each step as its own command, never chained after an edit or build.

1. **Guard.** Find the card (`findmnt -rn -o TARGET,OPTIONS | grep /run/media`), then require:
   mounted `rw`, `/sys/block/mmcblk0/ro` is `0`, `test -w "$C"`, and `$C/System/` exists. Any
   failure: stop and tell the owner (often the lock switch: reinsert).
2. **Sync.** It rebuilds first, so it needs the whole toolchain; it adds and replaces, and games,
   saves and config stay:
   ```sh
   nix-shell -p go-task rustup cmake jq zig cargo-zigbuild rsync file curl unzip bun --run \
     "CARGO_NET_GIT_FETCH_WITH_CLI=true task sync:card CARD=$C"
   ```
3. **Verify.** `cmp dist-device/System/slot "$C/System/slot" && sync`, then say it is safe to remove.
