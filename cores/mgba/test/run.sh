#!/bin/sh
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
src="${1:?usage: run.sh MGBA_SOURCE CORE [OLD_CORE]}"
core="${2:?usage: run.sh MGBA_SOURCE CORE [OLD_CORE]}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT HUP INT TERM
"${CC:-cc}" -I"$src/include" -o "$work/flash-bank" "$here/flash-bank.c" -ldl
if [ $# -ge 3 ]; then
	if "$work/flash-bank" "$3" > "$work/old.log" 2>&1; then
		echo 'old core passed; the regression check did not reproduce the bug' >&2
		exit 1
	fi
	grep -q 'after mask: selected flash bank is 0, expected 65536' "$work/old.log"
	cat "$work/old.log"
fi
"$work/flash-bank" "$core"
