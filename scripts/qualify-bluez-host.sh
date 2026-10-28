#!/usr/bin/env bash
# Read-only inventory. Never powers, pairs, scans, registers, or changes policy.
set -u
export LC_ALL=C
printf 'OpenContinuity P006 read-only host qualification\nauthorization: never_evaluated\n'
probe() {
    printf '\n$'
    printf ' %q' "$@"
    printf '\n'
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'NOT RUN: command unavailable\n'
        return
    fi
    timeout 5s "$@" 2>&1
    local status=$?
    if [ "$status" -ne 0 ]; then printf 'BLOCKED/UNKNOWN: exit %s\n' "$status"; fi
}
if [ -r /etc/os-release ]; then cat /etc/os-release; else printf 'distribution: UNKNOWN\n'; fi
probe uname -a
if command -v bluetoothd >/dev/null 2>&1; then probe bluetoothd --version
elif [ -x /usr/lib/bluetooth/bluetoothd ]; then probe /usr/lib/bluetooth/bluetoothd --version
else printf '\nBlueZ daemon version: UNKNOWN (bluetoothd unavailable)\n'; fi
probe id
probe busctl --system --no-pager --timeout=2s tree org.bluez
probe btmgmt info
probe btmgmt expinfo
shopt -s nullglob
adapters=(/sys/class/bluetooth/hci[0-9]*)
if [ "${#adapters[@]}" -eq 0 ]; then printf '\ncontroller inventory: NOT AVAILABLE in sysfs\n'; fi
for entry in "${adapters[@]}"; do
    adapter=${entry##*/}
    [[ "$adapter" =~ ^hci[0-9]+$ ]] || continue
    printf '\ncontroller: %s\n' "$adapter"
    probe readlink -f "$entry/device"
    for file in "$entry/device/modalias" "$entry/device/uevent" "$entry/device/firmware_node/path"; do
        if [ -r "$file" ]; then printf '\n%s:\n' "$file"; cat "$file"; fi
    done
    path="/org/bluez/$adapter"
    probe busctl --system --no-pager --timeout=2s introspect org.bluez "$path"
    for property in Powered Discoverable Pairable; do
        probe busctl --system --no-pager --timeout=2s get-property org.bluez "$path" org.bluez.Adapter1 "$property"
    done
    for property in SupportedInstances ActiveInstances SupportedIncludes SupportedSecondaryChannels SupportedCapabilities SupportedFeatures; do
        probe busctl --system --no-pager --timeout=2s get-property org.bluez "$path" org.bluez.LEAdvertisingManager1 "$property"
    done
done
printf '\nRegistration, over-the-air observation, discovery and frame exchange: NOT RUN by this read-only script.\n'
printf 'Firmware version and controller roles: UNKNOWN unless reported by the commands above.\n'
printf 'authorization: never_evaluated\n'
