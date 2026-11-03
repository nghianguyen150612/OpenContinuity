#!/usr/bin/env python3
"""Validate explicit local evidence; never access a radio or network.

Only fixed vocabulary and bounded counts leave this process. Input errors never
include input values, filenames, decoder excerpts, or exception details.
"""
import argparse
import json
import os
from pathlib import Path
import re
import sys
import stat

MAX_BYTES = 512 * 1024
MAX_LINE = 2048
COMMON = {'schema_version', 'source', 'type', 'authorization', 'scan_session_id', 'sequence'}
SESSION = COMMON | {'state', 'status', 'scan_started', 'requested_duration_seconds', 'adapter_index'}
DEVICE = COMMON | {'id', 'display_name', 'provenance', 'change'}
TERMINAL = COMMON | {'scan_requested', 'scan_started', 'scan_completed', 'requested_duration_seconds', 'actual_duration_ms', 'adapter_index', 'status', 'cleanup_status', 'device_count', 'device_event_count', 'termination_reason'}
REASONS = dict(completed='duration_elapsed', cancelled='user_cancelled', no_adapter='adapter_lost_or_unavailable', adapter_not_powered='adapter_power_lost_or_off', bluez_unavailable='bluez_owner_lost_or_unavailable', timeout='deadline_expired', data_limit='stream_or_input_limit', output_blocked='stdout_blocked', output_closed='stdout_closed', cleanup_incomplete='cleanup_failed')
STATUSES = set(REASONS) | {'unsupported_le', 'adapter_busy', 'system_bus_unavailable', 'access_denied', 'malformed_data', 'unsupported_platform'}


def require(condition):
    if not condition:
        raise ValueError('invalid evidence')


def integer(value, lo, hi):
    return type(value) is int and lo <= value <= hi


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result)
        result[key] = value
    return result


def decode(data):
    # Bound nesting before json.loads (including braces inside strings is safely
    # conservative). Protocol and preflight contain no deeply nested content.
    require(len(data) <= MAX_BYTES)
    depth = 0
    for byte in data:
        if byte in b'{[':
            depth += 1
            require(depth <= 8)
        elif byte in b'}]':
            depth -= 1
    return json.loads(data, object_pairs_hook=pairs,
                      parse_constant=lambda _: require(False))


def read_local(path):
    fd = os.open(Path(path), os.O_RDONLY | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        require(stat.S_ISREG(os.fstat(source.fileno()).st_mode))
        data = source.read(MAX_BYTES + 1)
    require(len(data) <= MAX_BYTES)
    return data


def preflight(data):
    obj = decode(data)
    require(type(obj) is dict)
    # P015 preflight is a legacy unversioned format; a supplied version must be 1.
    require(type(obj.get('schema_version', 1)) is int and obj.get('schema_version', 1) == 1)
    require(obj.get('system_bus') in {'available', 'blocked_or_unavailable'})
    adapters = obj.get('adapters')
    require(type(adapters) is list and len(adapters) <= 16)
    for index, adapter in enumerate(adapters, 1):
        require(type(adapter) is dict and set(adapter) == {'index', 'powered', 'transport_filter'})
        require(type(adapter['index']) is int and adapter['index'] == index)
        require(type(adapter['powered']) is bool or adapter['powered'] == 'unknown')
        require(type(adapter['transport_filter']) is bool or adapter['transport_filter'] == 'unknown')
    # No free-form environment/version strings or other input fields are echoed.
    return {'host_environment': 'local_preflight_supplied',
            'bluez_readiness': 'reported_ready' if obj['system_bus'] == 'available' and obj.get('bluez_access') == 'available' and any(a['powered'] is True and a['transport_filter'] is True for a in adapters) else 'blocked_or_unavailable'}


def transcript(data):
    require(data.endswith(b'\n'))
    lines = data.splitlines(keepends=True)
    require(2 <= len(lines) <= 514)
    session = None
    devices = {}
    event_count = 0
    terminal = None
    for sequence, raw in enumerate(lines, 1):
        require(len(raw) <= MAX_LINE)
        obj = decode(raw)
        require(type(obj) is dict and type(obj.get('schema_version')) is int and obj['schema_version'] == 1)
        require(obj.get('source') == 'bluez_le_discovery_events' and obj.get('authorization') == 'never_evaluated')
        require(type(obj.get('sequence')) is int and obj['sequence'] == sequence)
        sid = obj.get('scan_session_id')
        require(type(sid) is str and re.fullmatch('[0-9a-f]{32}', sid))
        require(terminal is None)
        if sequence == 1:
            require(set(obj) == SESSION and obj['type'] == 'session')
            require(type(obj['scan_started']) is bool and integer(obj['requested_duration_seconds'], 1, 30))
            require(obj['adapter_index'] is None or integer(obj['adapter_index'], 1, 16))
            if obj['scan_started']:
                require(obj['state'] == 'started' and obj['status'] == 'started' and obj['adapter_index'] is not None)
            else:
                require(obj['state'] == 'not_started' and obj['status'] in STATUSES)
            session = obj
            continue
        require(sid == session['scan_session_id'])
        if obj.get('type') == 'device':
            require(set(obj) == DEVICE and session['scan_started'] and event_count < 512)
            require(obj['provenance'] == 'bluez_event_during_session')
            ident, name = obj['id'], obj['display_name']
            require(type(ident) is str and type(name) is str and 1 <= len(name) <= 80 and name.strip())
            # Reject sensitive labels; never emit labels or ephemeral identifiers.
            require(not re.search(r'[/\\@]|(?:[0-9a-fA-F]{2}[:_\-]){5}[0-9a-fA-F]{2}|(?:\d{1,3}\.){3}\d{1,3}|(?:serial|cookie|token|secret|key)\s*[:=]', name, re.I))
            require(all(c.isprintable() for c in name))
            if obj['change'] == 'observed':
                require(len(devices) < 128 and ident == f"scan-{sid[:16]}-{len(devices)+1:03}" and ident not in devices)
            else:
                require(obj['change'] == 'updated' and ident in devices and devices[ident] != name)
            devices[ident] = name
            event_count += 1
        else:
            require(set(obj) == TERMINAL and obj.get('type') == 'terminal')
            require(obj['scan_requested'] is True and type(obj['scan_started']) is bool and obj['scan_started'] == session['scan_started'])
            require(obj['requested_duration_seconds'] == session['requested_duration_seconds'] and type(obj['requested_duration_seconds']) is int and obj['adapter_index'] == session['adapter_index'])
            require(obj['adapter_index'] is None or integer(obj['adapter_index'], 1, 16))
            require(integer(obj['actual_duration_ms'], 0, 40000))
            require(integer(obj['device_count'], 0, 128) and obj['device_count'] == len(devices))
            require(integer(obj['device_event_count'], 0, 512) and obj['device_event_count'] == event_count)
            status, cleanup = obj['status'], obj['cleanup_status']
            require(status in STATUSES and cleanup in {'complete', 'incomplete', 'not_needed'})
            require(obj['termination_reason'] == REASONS.get(status, 'setup_or_observation_failed'))
            require(type(obj['scan_completed']) is bool and obj['scan_completed'] == (status == 'completed' and cleanup == 'complete'))
            if status == 'completed':
                require(session['scan_started'] and cleanup == 'complete' and obj['actual_duration_ms'] >= session['requested_duration_seconds'] * 1000)
            if session['scan_started']:
                require(cleanup != 'not_needed')
            else:
                require(not devices and obj['actual_duration_ms'] == 0 and status == session['status'])
            require(status != 'cleanup_incomplete' or cleanup == 'incomplete')
            terminal = obj
    require(terminal is not None)
    return {'discovery_session': 'transcript_only_origin_unverified', 'stream_delivery': 'complete_transcript_exit_unverified',
            'terminal_status': terminal['status'], 'cleanup': 'reported_' + terminal['cleanup_status'],
            'device_count': len(devices), 'device_event_count': event_count, 'record_count': len(lines)}


def summarize(preflight_data, scan_data=None):
    result = {'schema_version': 1, 'authorization': 'never_evaluated',
              'discovery_session': 'not_run', 'stream_delivery': 'not_run', 'cleanup': 'not_run',
              'independent_rf': 'not_run', 'wayland_gui': 'not_run', 'apple_watch_se2': 'no_go'}
    result.update(preflight(preflight_data))
    if scan_data is not None:
        result.update(transcript(scan_data))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight', required=True)
    parser.add_argument('--transcript')
    args = parser.parse_args()
    try:
        result = summarize(read_local(args.preflight), read_local(args.transcript) if args.transcript else None)
    except (OSError, ValueError, TypeError, KeyError, RecursionError):
        print('Evidence rejected: invalid, unsafe, or unavailable local input.', file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0


if __name__ == '__main__':
    sys.exit(main())
