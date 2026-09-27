#!/usr/bin/env python3
"""Validate a fresh official API capture before Rust consumes its JSON."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
API = ROOT / 'tools/api-json'
SPEC = importlib.util.spec_from_file_location('independent_api_json', API / 'verify.py')
ORACLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ORACLE)
RUNNER_SOURCES = ('Dockerfile', '.dockerignore', 'inputs.json', 'fetch.py', 'spike.py', 'component.py', 'verify.py', 'run.py')
COMPONENT_SOURCES = ('spike.py', 'component.py', 'verify.py', 'inputs.json')


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
    def constant(_value):
        raise ValueError('nonfinite JSON constant')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=constant)


def load(path):
    with path.open('rb') as source:
        raw = source.read(8 * 1024 * 1024 + 1)
    require(len(raw) <= 8 * 1024 * 1024, 'capture JSON exceeds 8MiB')
    return strict_json(raw)


def current_hashes(names):
    return {name: hashlib.sha256((API / name).read_bytes()).hexdigest() for name in names}


def check(directory):
    fixture = load(directory / 'fixtures.json')
    result = load(directory / 'result.json')
    runner = load(directory / 'runner-result.json')
    frozen = load(API / 'fixtures.json')
    provenance = load(API / 'provenance.json')
    require(hashlib.sha256((API / 'fixtures.json').read_bytes()).hexdigest() ==
            provenance['durable_sha256']['fixtures.json'], 'reviewed frozen fixture hash mismatch')
    ORACLE.verify(fixture)
    require(ORACLE.typed_equal(fixture, frozen), 'fresh fixture differs from reviewed complete fixture')
    require(result.get('status') == 'passed', 'component capture failed')
    require('error' not in result and 'cleanup_error' not in result, 'capture has failure diagnostics')
    require(type(runner.get('exit_code')) is int and runner['exit_code'] == 0, 'runner did not exit zero')
    require(runner.get('errors') == [], 'runner cleanup or execution errors')
    require(ORACLE.typed_equal(runner.get('source_sha256'), current_hashes(RUNNER_SOURCES)), 'runner source hashes differ from current capture source')
    require(ORACLE.typed_equal(result.get('source_sha256'), current_hashes(COMPONENT_SOURCES)), 'executed source hashes differ from current capture source')
    require(ORACLE.typed_equal(result.get('inputs'), load(API / 'inputs.json')), 'component input pins differ')
    require(result.get('architecture') in ('arm64', 'amd64'), 'unknown capture architecture')
    require(type(result.get('kernel')) is str and bool(result['kernel']), 'missing environment kernel')
    require(ORACLE.typed_equal(result.get('fixture'), fixture), 'result and fixture file disagree')
    require(all('runtime digest ' + name in result.get('checks', []) for name in ('kube-apiserver', 'kine')), 'missing runtime binary digest checks')
    shutdowns = result.get('shutdowns')
    require(isinstance(shutdowns, list) and len(shutdowns) == 2, 'missing component shutdown evidence')
    require({s['component'] for s in shutdowns} == {'kube-apiserver', 'kine'}, 'wrong shutdown ownership')
    for shutdown in shutdowns:
        require({'component', 'exit_code', 'forced', 'owned_group_remained', 'error'} <= shutdown.keys(), 'incomplete shutdown record')
        require(type(shutdown.get('exit_code')) is int and shutdown['exit_code'] == 0, 'unclean component exit')
        require(shutdown.get('forced') is False and shutdown.get('owned_group_remained') is False
                and shutdown.get('error') is None, 'component cleanup incomplete')
    tls = result.get('datastore_tls')
    require(isinstance(tls, list) and len(tls) == 2 and {r['identity'] for r in tls} == {'absent', 'admin'}, 'missing dedicated datastore TLS negatives')
    for probe in tls:
        require(type(probe.get('exit_code')) is int and probe['exit_code'] == 1, 'TLS negative did not reject')
        diagnostic = probe.get('diagnostic', '').lower()
        expected = 'alert handshake failure' if probe['identity'] == 'absent' else 'alert unknown ca'
        require(expected in diagnostic, 'TLS probe failed for an unexpected reason')
    http = result.get('http')
    require(isinstance(http, list), 'missing raw HTTP observations')
    expected_status = {'anonymous-denied': 401, 'rbac-denied': 403, 'namespace': 201,
        'service-account': 201, 'pod-create': 201, 'pod-read': 200, 'service-create': 201,
        'service-read': 200, 'crd-create': 201, 'crd-ready': 200, 'custom-create': 201,
        'custom-read': 200, 'watch-before': 200, 'watch-create': 201, 'watch-update': 200, 'watch-delete': 200}
    records = {}
    for record in http:
        name = record['name']
        require(name in expected_status and type(record['status']) is int and record['status'] == expected_status[name], 'unexpected HTTP status')
        require(name not in records or name == 'crd-ready', 'duplicate HTTP scenario')
        parsed = strict_json(record['raw_response'])
        require(ORACLE.typed_equal(parsed, record['response']), 'raw HTTP and parsed observation disagree')
        records[name] = record

    require(records.keys() == expected_status.keys(), 'incomplete or unexpected HTTP scenario inventory')
    for name, expected in expected_status.items():
        require(type(records[name]['status']) is int and records[name]['status'] == expected, 'unexpected HTTP status: ' + name)
    for name, expected in fixture['cases'].items():
        require(ORACLE.typed_equal(ORACLE.normalize(records[name]['response']), expected), 'raw resource does not reproduce fixture: ' + name)
    lines = result.get('raw_watch_lines')
    require(isinstance(lines, list) and len(lines) == 3, 'missing raw watch events')
    events = [strict_json(line) for line in lines]
    normalized = [{'type': event['type'], 'object': ORACLE.normalize(event['object'])} for event in events]
    require(ORACLE.typed_equal(normalized, fixture['watch']), 'raw watch does not reproduce fixture')
    return {'status': 'verified', 'architecture': result['architecture'], 'resource_documents': len(fixture['cases']),
            'watch_events': len(events), 'fixture_sha256': hashlib.sha256((directory / 'fixtures.json').read_bytes()).hexdigest()}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture_dir', type=Path)
    args = parser.parse_args(argv)
    try:
        print(json.dumps(check(args.capture_dir), sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, TypeError, RecursionError):
        # Do not echo supplied raw responses, TLS details, or credential-like bytes on failure.
        print('API capture validation failed; inspect the owned capture and source identity locally', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
