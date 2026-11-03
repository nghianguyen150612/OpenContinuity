#!/usr/bin/env python3
"""Read-only, sanitized BlueZ preflight. No discovery or adapter changes."""
import json
import platform
import re
import shutil
import subprocess


def run(args):
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=3, check=False)
        return result.stdout if result.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired):
        return None


def os_release():
    result = {}
    try:
        with open("/etc/os-release", encoding="utf-8") as source:
            for line in source:
                key, sep, value = line.partition("=")
                value = value.strip().strip('"')
                if sep and key in ("ID", "VERSION_ID") and re.fullmatch(r"[A-Za-z0-9._-]{1,40}", value):
                    result[key.lower()] = value
    except OSError:
        pass
    return result


def preflight():
    report = {"schema_version": 1, "environment": {"system": platform.system(), "kernel": platform.release(), "os": os_release()},
              "bluez_version": "unavailable", "system_bus": "blocked_or_unavailable",
              "adapters": [], "cli_version": "unavailable", "settings": "unavailable",
              "discovery_session": "not_run", "stream_delivery": "not_run",
              "cleanup_confirmation": "not_run", "independent_rf": "not_run",
              "wayland_gui": "not_run", "apple_watch": "not_run"}
    version = run(["bluetoothd", "--version"])
    if version and re.fullmatch(r"[0-9.]+", version.strip()):
        report["bluez_version"] = version.strip()
    cli = run(["opencontinuity", "--version"])
    if cli and re.fullmatch(r"[a-zA-Z0-9 ._-]{1,80}", cli.strip()):
        report["cli_version"] = cli.strip()
    if shutil.which("opencontinuity-settings"):
        report["settings"] = "installed_version_unknown"
    bus = run(["busctl", "--system", "--no-pager", "list", "--no-legend"])
    if bus is None:
        return report
    report["system_bus"] = "available"
    tree = run(["busctl", "--system", "--no-pager", "tree", "org.bluez", "--list"])
    if tree is None:
        report["bluez_access"] = "blocked_or_unavailable"
        return report
    report["bluez_access"] = "available"
    paths = sorted(set(re.findall(r"^/org/bluez/hci[0-9]+$", tree, re.MULTILINE)))
    for path in paths:
        powered = run(["busctl", "--system", "get-property", "org.bluez", path,
                       "org.bluez.Adapter1", "Powered"])
        filters = run(["busctl", "--system", "call", "org.bluez", path,
                       "org.bluez.Adapter1", "GetDiscoveryFilters"])
        report["adapters"].append({"index": len(report["adapters"]) + 1,
                                   "powered": {"b true": True, "b false": False}.get((powered or "").strip(), "unknown"),
                                   "transport_filter": ("unknown" if filters is None else bool(re.search(r'"Transport"', filters)))})
    return report


if __name__ == "__main__":
    print(json.dumps(preflight(), indent=2))
