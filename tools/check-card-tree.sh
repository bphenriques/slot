#!/bin/sh
# Run against the exact tree that will be packaged for a device release.
set -eu
out="${1:?usage: check-card-tree.sh <card tree>}"

for binary in slot slot-services mgba_libretro.so gpsp_libretro.so fsck.exfat slot-net/iw; do
    file "$out/System/$binary" | grep -q aarch64
done
test -x "$out/System/slot"
test -x "$out/System/slot-services"
for config in wifi retroachievements; do
    test -f "$out/Config/$config.toml.example"
    test ! -e "$out/Config/$config.toml"
done
test ! -e "$out/Config/slot.state"
for folder in Games Guides Labels Saves States Cheats; do
    for platform in GBA GBAV GB GBC; do
        test -d "$out/$folder/$platform"
    done
done
for folder in Bezels Cheats Labels Wallpapers; do
    test -f "$out/$folder/README.txt"
done

gpsp_src_sha="$(cat vendor/gpsp-src.sha)"
test -f "$out/System/licenses/gpsp-${gpsp_src_sha}.tar.gz"
test -f "$out/System/licenses/gpsp-${gpsp_src_sha}.meta"
mgba_src_commit="$(sed -n 's/^commit=//p' vendor/mgba_libretro.so.meta)"
test -n "$mgba_src_commit"
test -f "$out/System/licenses/mgba-${mgba_src_commit}.meta"
for core in gpsp mgba; do
    for patch in cores/$core/*.patch; do
        test -f "$out/System/licenses/$core-$(basename "$patch")"
    done
done
echo "Device binaries, content folders, config examples and core sources verified in $out"
