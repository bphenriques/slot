#!/bin/sh
# Take Muse, plorpOS's audio player daemon, and the FFmpeg it links from a pinned plorpOS H700
# release. Nothing is built: the release's binaries already target BaseOS.
#   sh music/fetch-muse.sh RELEASE ZIP SHA256 [OUT]
set -eu
release=$1
zip=$2
sha=$3
out=${4:-vendor/muse}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM

curl -fsSL -o "$work/$zip" "https://github.com/thwonp/plorpOS/releases/download/$release/$zip"
got=$( (sha256sum "$work/$zip" 2>/dev/null || shasum -a 256 "$work/$zip") | cut -d' ' -f1)
[ "$got" = "$sha" ] || { echo "$zip: sha256 $got, want $sha" >&2; exit 1; }

unzip -q "$work/$zip" -d "$work/x" \
    'TortOS/muse' 'TortOS/musectl' 'TortOS/LICENSE' \
    'TortOS/lib/libav*' 'TortOS/lib/libswresample*' 'TortOS/lib/COPYING.LGPLv2.1'
src="$work/x/TortOS"
for f in muse musectl; do file "$src/$f" | grep -q aarch64; done

rm -rf "$out"
mkdir -p "$out/lib" "$out/licenses"
cp "$src/muse" "$src/musectl" "$out/"
cp "$src"/lib/*.so.* "$out/lib/"
cp "$src/LICENSE" "$out/licenses/muse-MIT.txt"
cp "$src/lib/COPYING.LGPLv2.1" "$out/licenses/ffmpeg-LGPL-2.1.txt"
cat > "$out/versions.txt" <<EOF
muse, musectl: plorpOS $release ($zip, sha256 $sha), https://github.com/thwonp/plorpOS/tree/$release/src/muse
ffmpeg 6.1, LGPL-2.1 only, built by plorpOS mk/fetch-h700-sysroot.sh at $release
source: https://ffmpeg.org/releases/ffmpeg-6.1.tar.gz (sha256 938dd778baa04d353163ca5cb06c909c918850055f549205b29b1224e45a5316)
EOF
