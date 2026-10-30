#!/usr/bin/env python3
"""Offline strict evidence manifest validator. Never operates Bluetooth.

Validation establishes structure and declared provenance, not truth of observations
or authenticity of signatures. Review referenced sanitized proofs independently.
"""
import argparse
import datetime
import json
import pathlib
import re

OUTCOMES = {'PASS', 'FAIL', 'BLOCKED', 'NOT RUN'}
STAGES = {
    'gatt_registration': 'transport', 'advertisement_registration': 'transport',
    'radio_advertisement': 'transport', 'watch_discovery': 'transport',
    'watch_characteristics': 'transport', 'challenge_read': 'transport',
    'acknowledged_writes': 'transport', 'result_read': 'transport',
    'p007_digest': 'crypto', 'cancellation': 'transport',
    'reconnect': 'transport', 'suspension': 'transport',
    'server_timeout': 'transport', 'cleanup': 'transport',
    'pinned_enrollment': 'crypto', 'host_signature': 'crypto',
    'request_delivery': 'transport', 'local_authentication': 'consent',
    'signed_rejection': 'consent', 'response_delivery': 'transport',
    'p009_verification': 'crypto',
}
LENGTHS = {'challenge_read': 25, 'result_read': 44}
ERRORS = {'none', 'prerequisite_unavailable', 'timeout', 'length_mismatch',
          'transport', 'crypto', 'consent', 'registration', 'cancelled'}
VERSIONS = {'linux', 'kernel', 'bluez', 'controller', 'firmware', 'watch_model',
            'watchos', 'xcode', 'app_revision', 'scanner'}


def timestamp(value):
    if not isinstance(value, str):
        raise ValueError('timestamp required')
    parsed = datetime.datetime.fromisoformat(value.replace('Z', '+00:00'))
    if parsed.utcoffset() != datetime.timedelta(0):
        raise ValueError('UTC timestamp required')


def keys(value, required):
    if not isinstance(value, dict) or set(value) != set(required):
        raise ValueError('unknown or missing fields; raw diagnostic text is forbidden')


def integer(value, maximum):
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError('numeric bound')


def template(source='host_preflight'):
    return {'schema_version': 1, 'authorization': 'never_evaluated', 'source': source,
            'versions': {key: 'unavailable' for key in sorted(VERSIONS)},
            'stages': [dict(stage=stage, domain=domain, outcome='BLOCKED',
                            timestamp_utc=None, expected_bytes=LENGTHS.get(stage),
                            actual_bytes=None, elapsed_ms=None, evidence_ref=None,
                            error_class='prerequisite_unavailable')
                       for stage, domain in STAGES.items()]}


def validate(doc, require_physical=False):
    keys(doc, {'schema_version', 'authorization', 'source', 'versions', 'stages'})
    if type(doc['schema_version']) is not int or doc['schema_version'] != 1 or doc['authorization'] != 'never_evaluated':
        raise ValueError('unsupported schema or authorization')
    if doc['source'] not in {'physical', 'simulated', 'host_preflight'}:
        raise ValueError('invalid provenance')
    keys(doc['versions'], VERSIONS)
    for value in doc['versions'].values():
        if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z0-9 ._+-]{1,80}', value):
            raise ValueError('sanitized software/model version required')
    if not isinstance(doc['stages'], list) or len(doc['stages']) != len(STAGES):
        raise ValueError('complete stage inventory required')
    seen = set()
    for row in doc['stages']:
        keys(row, {'stage', 'domain', 'outcome', 'timestamp_utc', 'expected_bytes',
                   'actual_bytes', 'elapsed_ms', 'evidence_ref', 'error_class'})
        stage = row['stage']
        if stage not in STAGES or stage in seen or row['domain'] != STAGES[stage]:
            raise ValueError('unknown, duplicate or misclassified stage')
        seen.add(stage)
        if row['outcome'] not in OUTCOMES or row['error_class'] not in ERRORS:
            raise ValueError('invalid outcome/error')
        for field, bound in [('expected_bytes', 65535), ('actual_bytes', 65535), ('elapsed_ms', 3600000)]:
            if row[field] is not None:
                integer(row[field], bound)
        if row['expected_bytes'] != LENGTHS.get(stage):
            raise ValueError('incorrect expected byte length')
        if row['timestamp_utc'] is not None:
            timestamp(row['timestamp_utc'])
        if row['evidence_ref'] is not None and (not isinstance(row['evidence_ref'], str) or
                not re.fullmatch(r'proof-[0-9]{3}\.json', row['evidence_ref'])):
            raise ValueError('use local opaque proof-NNN.json reference')
        executed = row['outcome'] in {'PASS', 'FAIL'}
        if executed and (row['timestamp_utc'] is None or row['evidence_ref'] is None):
            raise ValueError('executed stage requires timestamp and proof reference')
        if row['outcome'] == 'PASS':
            if row['error_class'] != 'none':
                raise ValueError('PASS with error')
            if stage in LENGTHS and row['actual_bytes'] != LENGTHS[stage]:
                raise ValueError('incomplete physical read')
        elif row['outcome'] in {'FAIL', 'BLOCKED'} and row['error_class'] == 'none':
            raise ValueError('failure/prerequisite classification required')
    if require_physical:
        if doc['source'] != 'physical' or any(r['outcome'] != 'PASS' for r in doc['stages']):
            raise ValueError('physical acceptance incomplete')
        if any(v == 'unavailable' for v in doc['versions'].values()):
            raise ValueError('physical acceptance requires device and tool versions')
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', nargs='?', type=pathlib.Path)
    parser.add_argument('--template', action='store_true')
    parser.add_argument('--require-physical', action='store_true')
    args = parser.parse_args()
    if args.template:
        print(json.dumps(template(), indent=2))
        return
    try:
        if args.input is None or args.input.stat().st_size > 65536:
            raise ValueError('supply a manifest up to 64 KiB')
        validate(json.loads(args.input.read_text()), args.require_physical)
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, f'Invalid evidence: {error}\n')
    print('PASS: manifest validation only; authorization: never_evaluated')


if __name__ == '__main__':
    main()
