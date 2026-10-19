#!/usr/bin/env bash
# Third-party notices for the Rust crates a package links: each one's license and notice files,
# as Cargo.lock has their versions. Usage: tools/third-party.sh <package> <title> > out.txt
set -euo pipefail
package=$1
title=$2
meta=$(cargo metadata --format-version 1 --locked)
echo "Third-party notices for $title"
echo
echo "Versions are recorded in Cargo.lock. These are the license and notice files supplied by the linked Rust dependencies."
echo
cargo tree --locked -p "$package" -e normal --prefix none --format '{p}' |
  sed 's/ (.*//; s/ v/ /' | sort -u | grep -v '^slot-' |
  while read -r name version; do
    dir=$(jq -r --arg n "$name" --arg v "$version" \
      '.packages[] | select(.name == $n and .version == $v) | .manifest_path' <<<"$meta" |
      xargs dirname)
    files=$(find "$dir" -maxdepth 1 -type f \( -iname 'LICEN[CS]E*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) | sort)
    if [ -z "$files" ]; then
      IFS=$'\t' read -r license repository < <(jq -r --arg n "$name" --arg v "$version" \
        '.packages[] | select(.name == $n and .version == $v) | "\(.license // "unknown")\t\(.repository // "crates.io")"' <<<"$meta")
      printf '\n========================================================================\n'
      printf '%s-%s\n' "$name" "$version"
      printf '========================================================================\n'
      printf 'License: %s (the crate supplies no license file)\nSource: %s\n' "$license" "$repository"
      case "$license" in *MPL-2.0*) echo "The MPL-2.0 text is in mgba-MPL-2.0.txt beside this file." ;; esac
      continue
    fi
    for f in $files; do
      printf '\n========================================================================\n'
      printf '%s-%s / %s\n' "$name" "$version" "$(basename "$f")"
      printf '========================================================================\n'
      cat "$f"
    done
  done
