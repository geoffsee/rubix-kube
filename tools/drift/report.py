#!/usr/bin/env python3
"""Compare explicit source/default snapshots without fetching or accepting new baselines."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys

from drift import strict_json

FILES = {'source': 'inventory.json.gz', 'defaults': 'defaults.json',
         'apiserver': 'apiserver-defaults.json'}
CATEGORIES = ('api_schema', 'protobuf_fields', 'protobuf_rpcs', 'component_defaults',
              'apiserver_defaults', 'feature_gates', 'snapshot_metadata')
MISSING = object()
NAMED_ARRAYS = {'files', 'messages', 'nested', 'fields', 'enums', 'values', 'services', 'methods', 'extensions', 'oneofs'}


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


def named_descriptor(value, collection=None):
    """Key named descriptor arrays by source identity, preserving all other values."""
    if collection in NAMED_ARRAYS and not isinstance(value, list):
        raise ValueError('invalid descriptor array: ' + collection)
    if isinstance(value, dict):
        return {key: named_descriptor(child, key) for key, child in value.items()}
    if isinstance(value, list):
        if not value and collection in NAMED_ARRAYS:
            return {}
        if collection in NAMED_ARRAYS and value and all(isinstance(item, dict) and isinstance(item.get('name'), list)
                         and len(item['name']) == 1 and type(item['name'][0]) is str for item in value):
            result = {}
            for item in value:
                name = item['name'][0]
                if name in result:
                    raise ValueError('duplicate named descriptor: ' + name)
                result[name] = named_descriptor(item)
            return result
        if collection in NAMED_ARRAYS:
            raise ValueError('invalid named descriptor entries: ' + collection)
        return [named_descriptor(child) for child in value]
    return value


def require_mapping(value, key):
    if not isinstance(value.get(key), dict):
        raise ValueError('missing or invalid object: ' + key)


def validate(snapshot):
    source = snapshot['source']
    for kind, document in snapshot.items():
        if not isinstance(document, dict) or type(document.get('schema_version')) is not int or document['schema_version'] != (2 if kind == 'source' else 1):
            raise ValueError('unsupported snapshot schema: ' + kind)
    for key in ('authority', 'inputs', 'openapi_definitions', 'cri', 'containerd'):
        require_mapping(source, key)
    for key in ('repository', 'commit'):
        if type(source['authority'].get(key)) is not str or not source['authority'][key]:
            raise ValueError('missing official source authority: ' + key)
    for key in ('cri', 'containerd'):
        files = source[key].get('files')
        if not isinstance(files, list):
            raise ValueError('missing descriptor files: ' + key)
        # Identity is required for stable reviewable RPC and message paths.
        if any(not isinstance(f, dict) or not isinstance(f.get('name'), list)
               or len(f['name']) != 1 or type(f['name'][0]) is not str for f in files):
            raise ValueError('invalid descriptor file identity: ' + key)
    for kind in ('defaults', 'apiserver'):
        document = snapshot[kind]
        for key in ('cases', 'registered_feature_gates'):
            require_mapping(document, key)
        for key in ('source_revision', 'go_version', 'platform', 'emulation_version', 'minimum_compatibility_version'):
            if type(document.get(key)) is not str or not document[key]:
                raise ValueError('missing default source pin: ' + kind + '/' + key)
    require_mapping(snapshot['apiserver'], 'apiserver_options')


def load_snapshot(directory):
    result = {}
    hashes = {}
    for kind, name in FILES.items():
        with (directory / name).open('rb') as stream:
            raw = stream.read(32 * 1024 * 1024 + 1)
        if len(raw) > 32 * 1024 * 1024:
            raise ValueError('snapshot exceeds 32MiB input limit: ' + name)
        hashes[name] = hashlib.sha256(raw).hexdigest()
        if name.endswith('.gz'):
            # Bound decompressed inventory size, even if a supplied archive has extreme expansion.
            with gzip.GzipFile(fileobj=io.BytesIO(raw)) as stream:
                raw = stream.read(32 * 1024 * 1024 + 1)
        if len(raw) > 32 * 1024 * 1024:
            raise ValueError('snapshot exceeds 32MiB limit: ' + name)
        result[kind] = strict_json(raw)
    validate(result)
    return result, hashes


def domains(snapshot):
    result = {category: {} for category in CATEGORIES}
    source = snapshot['source']
    result['api_schema'] = source['openapi_definitions']
    for protocol in ('cri', 'containerd'):
        descriptor = named_descriptor(source[protocol])
        rpc = {}
        for name, file in descriptor['files'].items() if isinstance(descriptor['files'], dict) else []:
            if 'services' in file:
                rpc[name] = file.pop('services')
        result['protobuf_fields'][protocol] = descriptor
        result['protobuf_rpcs'][protocol] = rpc
    for kind in ('defaults', 'apiserver'):
        document = snapshot[kind]
        result['component_defaults'][kind] = document['cases']
        result['feature_gates'][kind] = document['registered_feature_gates']
        result['snapshot_metadata'][kind] = {key: value for key, value in document.items()
            if key not in ('cases', 'registered_feature_gates') and not (kind == 'apiserver' and key == 'apiserver_options')}
    result['apiserver_defaults'] = snapshot['apiserver']['apiserver_options']
    result['snapshot_metadata']['source'] = {key: value for key, value in source.items()
        if key not in ('cri', 'containerd', 'openapi_definitions')}
    return result


def build_report(before, after, before_hashes=None, after_hashes=None):
    validate(before)
    validate(after)
    old = domains(before)
    new = domains(after)
    categorized = {category: list(changes(old[category], new[category])) for category in CATEGORIES}
    count = sum(map(len, categorized.values()))
    return {'schema_version': 1, 'status': 'changed' if count else 'unchanged',
            'change_count': count, 'removal_count': sum(c['kind'] == 'removed' for items in categorized.values() for c in items),
            'snapshot_sha256': {'before': before_hashes or {}, 'after': after_hashes or {}},
            'source_pins': {'before': old['snapshot_metadata'], 'after': new['snapshot_metadata']},
            'changes': categorized}


def markdown(report):
    lines = ['# Upstream adoption review', '',
             f"Status: **{report['status']}**; {report['change_count']} changes; {report['removal_count']} removals.", '',
             '## Source pins', '', '```json', json.dumps(report['source_pins'], sort_keys=True, indent=2), '```']
    for category in CATEGORIES:
        items = report['changes'][category]
        lines += ['', '## ' + category.replace('_', ' ').capitalize(), '']
        if not items:
            lines.append('No changes.')
        for item in items:
            # JSON representation makes newlines and control characters explicit.
            text = json.dumps(item, sort_keys=True, ensure_ascii=True)
            # An indented code block cannot be closed by content supplied in a snapshot.
            lines += ['    ' + text, '']
    return '\n'.join(lines).rstrip() + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--format', choices=('json', 'markdown'), default='json')
    args = parser.parse_args(argv)
    try:
        before, old_hashes = load_snapshot(args.before)
        after, new_hashes = load_snapshot(args.after)
        report = build_report(before, after, old_hashes, new_hashes)
        print(json.dumps(report, indent=2, sort_keys=True) if args.format == 'json' else markdown(report), end='\n' if args.format == 'json' else '')
        return 1 if report['change_count'] else 0
    except (OSError, EOFError, ValueError, KeyError, TypeError, RecursionError) as error:
        print('update-report: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
