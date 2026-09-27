#!/usr/bin/env python3
"""Independent source-derived filesystem assertions; no writer called as oracle."""
import json
from pathlib import Path
HERE = Path(__file__).resolve().parent
OLD = '# original bytes, preserve comments\nnetwork: {mtu: 1250}\n'

LIMIT = 1024 * 1024
REVISION = '2ef1c4787989f11f868f81bb84ae2afd4a49a81d'
ARCHIVE = '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec'
BUILDER = 'golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd'
HARNESS = {'Capture.Dockerfile', 'file_capture_test.go'}
LOCAL_FILES = {'Capture.Dockerfile', 'README.md', 'capture-receipt.json', 'capture.py',
               'evidence/build.log', 'evidence/first.log', 'evidence/repeat.json',
               'evidence/repeat.log', 'file_capture_test.go', 'reference.json',
               'replacement.yaml', 'test_links.py', 'verify.py'}
SOURCE_HASHES = {
    'internal/config/defaults.go': '902db154b17cd88d24f18b4f9aeb87d286470b5113c18b1f2066d20ab596a24b',
    'internal/config/file.go': '9a9af1ea2d94ff1fa1f2306f868721fb88f2e4b1d0d3fa286879b18de61749de',
    'types/config.go': 'e59764b93fd6689f091e79a17a633d94e910ebc23ae7db375734e212211ce4fa',
}

def require(condition, message):
    if not condition:
        raise ValueError(message)

def bounded(path, limit=LIMIT):
    with Path(path).open('rb') as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit, 'input exceeds bounded byte budget: ' + str(path))
    return data

def strict_json(data):
    import math
    def pairs(items):
        out = {}
        for key, value in items:
            require(key not in out, 'duplicate JSON key: ' + key)
            out[key] = value
        return out
    def floating(value):
        result = float(value)
        require(math.isfinite(result), 'nonfinite JSON number')
        return result
    def constant(value):
        raise ValueError('nonfinite JSON constant: ' + value)
    require(len(data) <= LIMIT, 'JSON exceeds bounded byte budget')
    return json.loads(data, object_pairs_hook=pairs, parse_float=floating,
                      parse_constant=constant)

def load(path):
    return strict_json(bounded(path))

def digest(path, limit=LIMIT):
    import hashlib
    return hashlib.sha256(bounded(path, limit)).hexdigest()

def keys(value, expected, label):
    require(type(value) is dict and set(value) == expected, label + ' inventory differs')

def equal(actual, expected, label):
    require(json.dumps(actual, sort_keys=True, allow_nan=False) ==
            json.dumps(expected, sort_keys=True, allow_nan=False), label + ' differs')

def raw_records(path):
    records = [strict_json(line[len(b'RUBIX_CAPTURE '):]) for line in bounded(path).splitlines()
               if line.startswith(b'RUBIX_CAPTURE ')]
    verify(records)
    return records

def expected():
    replacement = (HERE / 'replacement.yaml').read_text()
    def regular(data, mode='0644'):
        return {'kind': 'regular', 'mode': mode, 'bytes': data}
    out = []
    for name in ['destination-symlink', 'backup-symlink', 'backup-hardlink', 'permissive-backup', 'destination-directory', 'backup-directory', 'ordinary']:
        failed = name in ['destination-directory', 'backup-directory']
        entries = {'config.yaml': regular(replacement, '0600'), 'config.yaml.bak': regular(OLD, '0600')}
        if name == 'destination-symlink':
            entries['other'] = regular(OLD)
        elif name == 'backup-symlink':
            entries['config.yaml.bak'] = {'kind': 'symlink', 'mode': '0777', 'bytes': OLD, 'link': 'other'}
            entries['other'] = regular(OLD)
        elif name == 'backup-hardlink':
            entries['config.yaml.bak'] = regular(OLD)
            entries['other'] = regular(OLD)
        elif name == 'permissive-backup':
            entries['config.yaml.bak'] = regular(OLD)
        elif name == 'destination-directory':
            entries = {'config.yaml': {'kind': 'directory', 'mode': '0700'}}
        elif name == 'backup-directory':
            entries = {'config.yaml': regular(OLD), 'config.yaml.bak': {'kind': 'directory', 'mode': '0700'}}
        out.append({'case': name, 'write_error': failed, 'destination_inode_preserved': failed,
                    'backup_other_same_inode': name == 'backup-hardlink', 'entries': entries})
    return out

def verify(records):
    # Canonical JSON preserves bool/int distinctions that Python equality loses.
    canonical = lambda value: json.dumps(value, sort_keys=True, ensure_ascii=True, allow_nan=False)
    if canonical(records) != canonical(expected()):
        raise ValueError('filesystem semantics differ from independent expectations')

def verify_capture(directory, *, frozen=False):
    import re
    directory = Path(directory)
    receipt = load(directory / ('capture-receipt.json' if frozen else 'receipt.json'))
    keys(receipt, {'schema', 'reference_revision', 'source_archive_sha256', 'builder',
                   'capture_driver_sha256', 'verifier_sha256', 'replacement_sha256',
                   'harness_sha256', 'runs', 'cleanup_errors', 'image_id', 'repeat_equal',
                   'remaining_containers', 'remaining_images'}, 'receipt')
    equal(receipt['schema'], 1, 'receipt schema')
    for key, value in [('reference_revision', REVISION), ('source_archive_sha256', ARCHIVE),
                       ('builder', BUILDER)]:
        equal(receipt[key], value, key)
    require(type(receipt['image_id']) is str and
            re.fullmatch(r'sha256:[0-9a-f]{64}', receipt['image_id']), 'invalid image ID')
    for key, name in [('capture_driver_sha256', 'capture.py'), ('verifier_sha256', 'verify.py'),
                      ('replacement_sha256', 'replacement.yaml')]:
        equal(receipt[key], digest(HERE / name), key)
    keys(receipt['harness_sha256'], HARNESS, 'harness')
    for name in HARNESS:
        equal(receipt['harness_sha256'][name], digest(HERE / name), name)
    keys(receipt['runs'], {'first', 'repeat'}, 'runs')
    observations = []
    for run in ('first', 'repeat'):
        entry = receipt['runs'][run]
        keys(entry, {'exit_code', 'stdout_sha256'}, run)
        equal(entry['exit_code'], 0, run + ' exit code')
        log = directory / ('evidence' if frozen else '') / (run + '.log')
        equal(entry['stdout_sha256'], digest(log), run + ' raw digest')
        records = raw_records(log)
        output = directory / ('reference.json' if frozen and run == 'first' else
                              ('evidence/' if frozen else '') + run + '.json')
        equal(load(output), records, run + ' raw to normalized records')
        observations.append(records)
    equal(observations[0], observations[1], 'repeat records')
    require(receipt['repeat_equal'] is True, 'repeat not successful')
    for key in ('cleanup_errors', 'remaining_containers', 'remaining_images'):
        equal(receipt[key], [], key)
    return receipt

def verify_frozen():
    provenance = load(HERE / 'provenance.json')
    keys(provenance, {'schema', 'reference_revision', 'reference_source_sha256',
                      'expected_replacement_origin', 'local_sha256'}, 'provenance')
    equal(provenance['schema'], 1, 'provenance schema')
    equal(provenance['reference_revision'], REVISION, 'source revision')
    equal(provenance['reference_source_sha256'], SOURCE_HASHES, 'source inventory')
    equal(provenance['expected_replacement_origin'], {
        'path': 'tools/parity/fixtures/config-api/config.json', 'selector': '[0].first',
        'sha256': 'f71e045a1317753738eb44965ae6ffbb83c886d904e06cc5f3b6b66e09302800',
    }, 'replacement origin')
    origin = HERE.parent / 'config-api' / 'config.json'
    equal(digest(origin), provenance['expected_replacement_origin']['sha256'], 'origin digest')
    equal(load(origin)[0]['first'], bounded(HERE / 'replacement.yaml').decode(), 'replacement bytes')
    keys(provenance['local_sha256'], LOCAL_FILES, 'local provenance')
    for name in LOCAL_FILES:
        equal(provenance['local_sha256'][name],
              digest(HERE / name, 16 * LIMIT if name == 'evidence/build.log' else LIMIT), name)
    verify_capture(HERE, frozen=True)

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    args = parser.parse_args()
    verify_capture(args.capture)
    print('PASS: seven filesystem cases, two identical executions, verified cleanup and provenance')
