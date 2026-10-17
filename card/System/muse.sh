#!/bin/sh
# Muse, plorpOS's music daemon, which slot plays through unless Config/music.txt says
# `player = slot`. launch_frontend.sh starts it.
#   sh muse.sh start
#   sh muse.sh ctl 'PLAY\tpath=/mnt/sdcard/Music/x.flac' [...]
#   sh muse.sh stop
SD="${SLOT_ROOT:-/mnt/sdcard}"
DIR="$SD/System/muse"
LOADER=/lib/ld-linux-aarch64.so.1
PID="${AGS_RUN:-/run}/slot-muse.pid"

case "$1" in
start)
	grep -qiE '^[[:space:]]*player[[:space:]]*=[[:space:]]*slot[[:space:]]*$' "$SD/Config/music.txt" 2>/dev/null && exit 0
	pid="$(cat "$PID" 2>/dev/null)"
	[ -n "$pid" ] && [ -d "/proc/$pid" ] && exit 0
	[ -f "$SD/muse.log" ] && mv -f "$SD/muse.log" "$SD/muse.log.1"
	"$LOADER" --library-path "$DIR/lib" "$DIR/muse" > "$SD/muse.log" 2>&1 &
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
