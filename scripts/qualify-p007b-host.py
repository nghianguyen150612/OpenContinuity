#!/usr/bin/env python3
"""Read-only, bounded probes. Output deliberately excludes raw command output/IDs.

PASS below means the diagnostic command completed, never RF interoperability.
Use the older qualify-bluez-host.sh privately for detailed operator inspection.
"""
import datetime
import json
import pathlib
import platform
import shutil
import subprocess

PROBES = {
    "kernel_inventory": ["uname", "-a"],
    "bluez_cli_version": ["bluetoothctl", "--version"],
    "controller_list": ["bluetoothctl", "list"],
    "bluetooth_service": ["systemctl", "--no-pager", "status", "bluetooth"],
    "management_info": ["btmgmt", "info"],
    "bluez_dbus_tree": ["busctl", "--system", "--no-pager", "--timeout=2s", "tree", "org.bluez"],
}


def probe(command, which=shutil.which, run=subprocess.run):
    if not which(command[0]):
        return {"status": "BLOCKED", "reason": "command_unavailable"}
    try:
        result = run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                     timeout=5, check=False)
        return {"status": "PASS" if result.returncode == 0 else "FAIL",
                "exit_code": result.returncode}
    except subprocess.TimeoutExpired:
        return {"status": "FAIL", "reason": "probe_timeout"}
    except OSError:
        return {"status": "BLOCKED", "reason": "execution_unavailable"}


def main():
    probes = {name: probe(command) for name, command in PROBES.items()}
    detailed = pathlib.Path(__file__).resolve().with_name("qualify-bluez-host.sh")
    probes["existing_host_qualification"] = probe(["bash", str(detailed)])
    print(json.dumps({
        "schema_version": 1,
        "authorization": "never_evaluated",
        "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "kernel_release": platform.release(),
        "machine": platform.machine(),
        "probes": probes,
        "interpretation": "command_results_only_not_hardware_qualification",
        "physical_trial": "NOT RUN",
        "raw_output": "discarded_identifiers_not_published",
    }, indent=2))


if __name__ == "__main__":
    main()
