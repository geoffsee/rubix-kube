#!/usr/bin/env python3
"""Compare resolved snapshots: --before DIR --after DIR [--format json|markdown].

Each directory contains resolved.json (a copy of capture run0.json) and receipt.json.
The receipt's run0.json digest must bind the copied bytes exactly; its run1 digest
must agree. No capture, fetch, or baseline update occurs. Exit 0 unchanged, 1 changed,
2 malformed/unbound. Root report integration can merge CATEGORIES, changes,
source_pins, and snapshot_sha256 from build_report(load_snapshot(...), ...).
Receipts are provenance records, not signed attestations of an untrusted host.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import sys

LIMIT = 32 * 1024 * 1024
CATEGORIES = ('resolved_apiserver_options', 'resolved_completion_errors',
              'resolved_snapshot_metadata')
MISSING = object()
CONTROLS = {'bind_address': str, 'external_address': str, 'cert_directory': str,
            'operation': str, 'server_started': bool}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def strict_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result

    def invalid(_):
        raise ValueError('nonfinite JSON number')

    def finite(value):
        result = float(value)
        require(math.isfinite(result), 'nonfinite JSON number')
        return result

    return json.loads(raw, object_pairs_hook=pairs, parse_constant=invalid, parse_float=finite)


def read_json(path):
    with path.open('rb') as stream:
        raw = stream.read(LIMIT + 1)
    require(len(raw) <= LIMIT, 'snapshot exceeds input limit')
    return strict_json(raw), hashlib.sha256(raw).hexdigest()


def mapping(value, label):
    require(type(value) is dict, 'invalid object: ' + label)
    return value


def nonempty(value, label):
    require(type(value) is str and bool(value), 'invalid identity: ' + label)


def digest(value, label):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None,
            'invalid digest: ' + label)


def validate_record(record):
    mapping(record, 'record')
    require(type(record.get('schema_version')) is int and record['schema_version'] == 1,
            'unsupported resolved schema')
    runtime = mapping(record.get('runtime'), 'runtime')
    require(runtime.keys() == {'go', 'os', 'arch'}, 'runtime inventory')
    for key, value in runtime.items():
        nonempty(value, 'runtime/' + key)
    controls = mapping(record.get('controls'), 'controls')
    require(controls.keys() == CONTROLS.keys(), 'control inventory')
    for key, kind in CONTROLS.items():
        require(type(controls[key]) is kind, 'control type: ' + key)
    cases = mapping(record.get('cases'), 'cases')
    require(bool(cases), 'empty case inventory')
    for name, case in cases.items():
        nonempty(name, 'case name')
        mapping(case, 'case/' + name)
        if 'error' in case:
            require(case.keys() == {'error'}, 'mixed error/success case: ' + name)
            nonempty(case['error'], 'case error')
        else:
            for key in ('flags_before', 'flags_after'):
                flags = mapping(case.get(key), name + '/' + key)
                require(bool(flags), 'empty flag inventory')
                for flag, value in flags.items():
                    nonempty(flag, 'flag name')
                    require(type(value) is str, 'non-string flag value')


def validate_receipt(receipt, record_hash):
    mapping(receipt, 'receipt')
    for name in ('errors', 'cleanup_errors', 'remaining_containers', 'remaining_images'):
        require(type(receipt.get(name)) is list and receipt[name] == [],
                'unsuccessful or missing receipt status: ' + name)
    require(receipt.get('identical_repeats') is True, 'missing successful repeat check')
    outputs = mapping(receipt.get('output_sha256'), 'output hashes')
    for name in ('run0.json', 'run1.json'):
        digest(outputs.get(name), name)
        require(outputs[name] == record_hash, 'resolved.json does not match receipt ' + name)
    inputs = mapping(receipt.get('inputs'), 'inputs')
    require(type(inputs.get('schema_version')) is int and inputs['schema_version'] == 1,
            'unsupported input identity schema')
    for kind in ('source', 'go'):
        pin = mapping(inputs.get(kind), 'inputs/' + kind)
        digest(pin.get('sha256'), kind)
        require(type(pin.get('bytes')) is int and pin['bytes'] > 0, 'invalid archive length')
        nonempty(pin.get('url'), 'archive URL')
        require(pin['url'].startswith('https://'), 'archive URL must identify HTTPS source')
    revision = inputs['source'].get('revision')
    require(type(revision) is str and re.fullmatch('[0-9a-f]{40}', revision) is not None,
            'invalid official revision')
    nonempty(inputs['go'].get('version'), 'Go version')
    nonempty(inputs['go'].get('platform'), 'Go platform')
    sources = mapping(receipt.get('source_sha256'), 'harness source hashes')
    require(bool(sources), 'empty harness source inventory')
    for name, value in sources.items():
        nonempty(name, 'harness source identity')
        digest(value, name)
    digest(receipt.get('helper_sha256'), 'lifecycle helper')


def load_snapshot(directory):
    record, record_hash = read_json(Path(directory) / 'resolved.json')
    receipt, receipt_hash = read_json(Path(directory) / 'receipt.json')
    validate_record(record)
    validate_receipt(receipt, record_hash)
    require(record['runtime']['go'] == receipt['inputs']['go']['version'], 'runtime/toolchain version mismatch')
    require(record['runtime']['os'] + '/' + record['runtime']['arch'] == receipt['inputs']['go']['platform'],
            'runtime/toolchain platform mismatch')
    return {'record': record,
            'pins': {key: receipt[key] for key in ('inputs', 'source_sha256', 'helper_sha256')},
            'hashes': {'resolved.json': record_hash, 'receipt.json': receipt_hash}}


def pointer(path, key):
    return path + '/' + str(key).replace('~', '~0').replace('/', '~1')


def changes(before, after, path=''):
    if before is MISSING:
        yield {'path': path, 'kind': 'added', 'after': after}
    elif after is MISSING:
        yield {'path': path, 'kind': 'removed', 'before': before}
    elif type(before) is not type(after):
        yield {'path': path, 'kind': 'type_changed', 'before': before, 'after': after}
    elif isinstance(before, dict):
        for key in sorted(before.keys() | after.keys()):
            yield from changes(before.get(key, MISSING), after.get(key, MISSING), pointer(path, key))
    elif isinstance(before, list):
        for index in range(max(len(before), len(after))):
            yield from changes(before[index] if index < len(before) else MISSING,
                               after[index] if index < len(after) else MISSING, pointer(path, index))
    elif before != after:
        yield {'path': path, 'kind': 'changed', 'before': before, 'after': after}


def domains(snapshot):
    record = snapshot['record']
    validate_record(record)
    return {'resolved_apiserver_options': {name: case for name, case in record['cases'].items()
                                          if 'error' not in case},
            'resolved_completion_errors': {name: case for name, case in record['cases'].items()
                                          if 'error' in case},
            'resolved_snapshot_metadata': {'source_pins': snapshot['pins'],
                                          'record': {key: value for key, value in record.items()
                                                     if key != 'cases'}}}


def build_report(before, after):
    """Compare two load_snapshot results; do not omit a missing resolved snapshot."""
    old, new = domains(before), domains(after)
    categorized = {category: list(changes(old[category], new[category])) for category in CATEGORIES}
    count = sum(map(len, categorized.values()))
    return {'schema_version': 1, 'status': 'changed' if count else 'unchanged',
            'change_count': count,
            'removal_count': sum(change['kind'] == 'removed' for items in categorized.values()
                                 for change in items),
            'snapshot_sha256': {'before': before['hashes'], 'after': after['hashes']},
            'source_pins': {'before': before['pins'], 'after': after['pins']},
            'changes': categorized}


def markdown(report):
    lines = ['# Resolved API-server option review', '',
             f"Status: **{report['status']}**; {report['change_count']} changes; {report['removal_count']} removals.",
             '', '## Source pins', '', '```json',
             json.dumps(report['source_pins'], indent=2, sort_keys=True), '```']
    for category in CATEGORIES:
        lines += ['', '## ' + category.replace('_', ' ').capitalize(), '']
        items = report['changes'][category]
        if not items:
            lines.append('No changes.')
        for change in items:
            lines += ['    ' + json.dumps(change, sort_keys=True, ensure_ascii=True), '']
    return '\n'.join(lines).rstrip() + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--format', choices=('json', 'markdown'), default='json')
    args = parser.parse_args(argv)
    try:
        result = build_report(load_snapshot(args.before), load_snapshot(args.after))
        print(json.dumps(result, indent=2, sort_keys=True) if args.format == 'json'
              else markdown(result), end='\n' if args.format == 'json' else '')
        return int(result['status'] == 'changed')
    except (OSError, ValueError, KeyError, TypeError, RecursionError) as error:
        print('resolved-report: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
