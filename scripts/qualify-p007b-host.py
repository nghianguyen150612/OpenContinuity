#!/usr/bin/env python3
"""Read-only, bounded probes. Output deliberately excludes raw command output/IDs.

PASS below means the diagnostic command completed, never RF interoperability.
Use the older qualify-bluez-host.sh privately for detailed operator inspection.
"""
import argparse
import datetime
import json
import pathlib
import platform
import re
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


def adapter_probes(adapter):
    """Read-only properties. Successful introspection is not registration permission."""
    if adapter is None:
        return {name: {"status": "BLOCKED", "reason": "adapter_not_selected"}
                for name in ("selected_adapter", "powered", "gatt_manager", "advertising_manager",
                             "supported_instances", "active_instances")}
    if not re.fullmatch(r'hci[0-9]{1,4}', adapter):
        raise ValueError('adapter must be hci followed by up to four digits')
    base = ["busctl", "--system", "--no-pager", "--timeout=2s"]
    path = f"/org/bluez/{adapter}"
    result = {"selected_adapter": probe(base + ["introspect", "org.bluez", path, "org.bluez.Adapter1"])}
    for name, interface in (("gatt_manager", "org.bluez.GattManager1"),
                            ("advertising_manager", "org.bluez.LEAdvertisingManager1")):
        result[name] = probe(base + ["introspect", "org.bluez", path, interface])
    for name, interface, prop in (("powered", "org.bluez.Adapter1", "Powered"),
                                   ("supported_instances", "org.bluez.LEAdvertisingManager1", "SupportedInstances"),
                                   ("active_instances", "org.bluez.LEAdvertisingManager1", "ActiveInstances")):
        # Keep identifiers discarded, just like existing probes. Inspect values privately.
        result[name] = probe(base + ["get-property", "org.bluez", path, interface, prop])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adapter', help='Explicit hciN selection; never powers/configures it')
    args = parser.parse_args()
    try:
        selected = adapter_probes(args.adapter)
    except ValueError as error:
        parser.error(str(error))
    probes = {name: probe(command) for name, command in PROBES.items()}
    probes.update(selected)
    probes["system_dbus"] = probe(["busctl", "--system", "--no-pager", "--timeout=2s", "list"])
    for name in ("firmware_and_le_support", "application_registration_permission", "advertisement_registration_permission"):
        probes[name] = {"status": "NOT RUN", "reason": "requires_private_operator_inspection_or_approved_trial"}
    detailed = pathlib.Path(__file__).resolve().with_name("qualify-bluez-host.sh")
    probes["existing_host_qualification"] = probe(["bash", str(detailed)])
    print(json.dumps({
        "schema_version": 1,
        "authorization": "never_evaluated",
        "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source": "host_preflight",
        "selected_adapter": args.adapter,
        "distribution": platform.freedesktop_os_release().get("ID", "unknown"),
        "kernel_release": platform.release(),
        "machine": platform.machine(),
        "probes": probes,
        "interpretation": "command_results_only_not_hardware_qualification",
        "physical_trial": "NOT RUN",
        "raw_output": "discarded_identifiers_not_published",
    }, indent=2))


if __name__ == "__main__":
    main()
