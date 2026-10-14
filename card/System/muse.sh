#!/bin/sh
# Muse by hand, until slot drives it.
#   sh muse.sh start
#   sh muse.sh ctl 'PLAY\tpath=/mnt/sdcard/Music/x.flac' [...]
#   sh muse.sh stop
SD="${SLOT_ROOT:-/mnt/sdcard}"
DIR="$SD/System/muse"
LOADER=/lib/ld-linux-aarch64.so.1
PID=/tmp/muse.pid

case "$1" in
start)
	"$LOADER" --library-path "$DIR/lib" "$DIR/muse" >> "$SD/muse.log" 2>&1 &
	echo $! > "$PID"
	;;
stop)
	[ -f "$PID" ] && kill "$(cat "$PID")" 2>/dev/null
	rm -f "$PID"
	;;
ctl)
	shift
	"$LOADER" "$DIR/musectl" "$@"
	;;
*)
	echo "usage: $0 start | stop | ctl LINE..." >&2
	exit 2
	;;
esac
