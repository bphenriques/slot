#!/bin/sh
set -u

SD="${SLOT_ROOT:-/mnt/sdcard}"
PSY="${AGS_PSY:-/sys/class/power_supply/axp2202-battery}"
SLOT_STATE="${AGS_SLOT_STATE:-$SD/Config/slot.state}"
TRACE="${AGS_TRACE:-$SD/Logs/ags-shutdown-trace.log}"
CLOSED="${AGS_LID_CLOSED:-/run/slot-lid.closed}"
# The time slot last said it is busy with the lid closed (music playing on); while that is
# recent, the lid counts as just closed, so a pocket of music is never cut off.
BUSY="${AGS_LID_BUSY:-/run/slot-busy}"
BUSY_FOR="${AGS_LID_BUSY_FOR:-30}"
UPTIME="${AGS_UPTIME:-/proc/uptime}"
INTERVAL="${AGS_LID_INTERVAL:-5}"
GRACE="${AGS_LID_GRACE:-60}"
RMMOD="${AGS_RMMOD:-rmmod}"
SYNC="${AGS_SYNC:-sync}"
POWEROFF="${AGS_POWEROFF:-poweroff}"
SLEEP="${AGS_SLEEP:-sleep}"
WIFI_WAIT="${AGS_WIFI_WAIT:-5}"

now() {
	read -r up _ < "$UPTIME" 2>/dev/null || up=0
	echo "${up%%.*}"
}

busy() {
	b="$(cat "$BUSY" 2>/dev/null)"
	case "$b" in
	"" | *[!0-9]*) return 1 ;;
	esac
	[ $(($1 - b)) -le "$BUSY_FOR" ]
}

minutes() {
	m="$(sed -n 's/^lid_off_min=//p' "$SLOT_STATE" 2>/dev/null | tail -1)"
	case "$m" in
	1 | 3 | 5 | 10 | 15 | 30) echo "$m" ;;
	*) echo 3 ;;
	esac
}

unload_wifi() {
	$RMMOD 8821cs 2>/dev/null &
	waited=0
	while kill -0 $! 2>/dev/null && [ "$waited" -lt "$WIFI_WAIT" ]; do
		$SLEEP 1
		waited=$((waited + 1))
	done
}

power_off() {
	echo "$(date +%s) lid: closed ${1}s and slot has not powered off, powering off" >> "$TRACE" 2>/dev/null
	$SYNC
	unload_wifi
	$SYNC
	$POWEROFF
}

check() {
	if [ "$(cat "$PSY/hallkey" 2>/dev/null)" != 0 ]; then
		rm -f "$CLOSED"
		return 1
	fi
	t="$(now)"
	if busy "$t"; then
		echo "$t" > "$CLOSED"
		return 1
	fi
	if [ ! -s "$CLOSED" ]; then
		echo "$t" > "$CLOSED"
		return 1
	fi
	shut=$((t - $(cat "$CLOSED")))
	[ "$shut" -ge $(($(minutes) * 60 + GRACE)) ] || return 1
	rm -f "$CLOSED"
	power_off "$shut"
}

case "${1:-loop}" in
once)
	check || true
	;;
loop)
	while :; do
		check && exit 0
		sleep "$INTERVAL"
	done
	;;
*)
	echo "usage: lid.sh [once|loop]" >&2
	exit 2
	;;
esac
