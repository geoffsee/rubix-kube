"""Mutation evidence across independent schema, protocol, default and gate domains."""
import copy
import gzip
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import report

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def snapshot():
    source = {'schema_version': 2, 'authority': {'repository': 'https://example.invalid/kubernetes', 'commit': 'a' * 40},
        'inputs': {'cri': {'sha256': 'b' * 64}},
        'openapi_definitions': {'Example': {'properties': {'field': {'type': 'string'}}, 'required': ['field']}},
        'cri': {'files': [{'name': ['api.proto'], 'messages': [{'name': ['Message'], 'fields': [
            {'name': ['field'], 'number': [1], 'type': [9]}]}], 'services': [{'name': ['Runtime'], 'methods': [
            {'name': ['Read'], 'input': ['Message'], 'output': ['Message'], 'server_streaming': [1]}]}]}]},
        'containerd': {'files': [{'name': ['content.proto'], 'services': [{'name': ['Content'], 'methods': [
            {'name': ['Write'], 'input': ['WriteRequest'], 'output': ['WriteResponse'], 'client_streaming': [1]}]}]}]}}
    base = {'schema_version': 1, 'source_revision': 'a' * 40, 'go_version': 'go1.26.8', 'platform': 'linux/arm64',
            'emulation_version': '1.35', 'minimum_compatibility_version': '1.34',
            'cases': {'zero': {'kubelet': {'enabled': False, 'nested': {'value': 17}}}},
            'registered_feature_gates': {'Gate': {'enabled': True, 'specs': [{'default': True, 'version': '1.35'}]}}}
    api = copy.deepcopy(base)
    api['apiserver_options'] = {'flags': {'secure-port': {'default': '6443'}}}
    return {'source': source, 'defaults': base, 'apiserver': api}


def write_snapshot(directory, value):
    for kind, name in report.FILES.items():
        data = json.dumps(value[kind], sort_keys=True).encode()
        (directory / name).write_bytes(gzip.compress(data, mtime=0) if name.endswith('.gz') else data)


class ReportTests(unittest.TestCase):
    def test_unchanged_is_clean_and_deterministic_without_mutating_inputs(self):
        before = snapshot()
        saved = copy.deepcopy(before)
        result = report.build_report(before, copy.deepcopy(before))
        self.assertEqual(result['status'], 'unchanged')
        self.assertEqual(result['change_count'], 0)
        self.assertEqual(report.markdown(result), report.markdown(report.build_report(before, before)))
        self.assertEqual(before, saved)

    def test_every_requested_domain_detects_independent_mutation(self):
        for domain in ('api_schema', 'protobuf_fields', 'protobuf_rpcs', 'component_defaults', 'apiserver_defaults', 'feature_gates'):
            before = snapshot()
            after = copy.deepcopy(before)
            if domain == 'api_schema': after['source']['openapi_definitions']['Example']['properties']['field']['type'] = 'integer'
            elif domain == 'protobuf_fields': after['source']['cri']['files'][0]['messages'][0]['fields'][0]['number'] = [2]
            elif domain == 'protobuf_rpcs': after['source']['containerd']['files'][0]['services'][0]['methods'][0]['client_streaming'] = [0]
            elif domain == 'component_defaults': after['defaults']['cases']['zero']['kubelet']['nested']['value'] = 19
            elif domain == 'apiserver_defaults': after['apiserver']['apiserver_options']['flags']['secure-port']['default'] = '443'
            else: after['defaults']['registered_feature_gates']['Gate']['specs'][0]['default'] = False
            result = report.build_report(before, after)
            with self.subTest(domain=domain):
                self.assertEqual(result['status'], 'changed')
                self.assertEqual(result['change_count'], 1)
                self.assertTrue(result['changes'][domain])

    def test_cri_rpc_signatures_and_named_removal_have_readable_identity(self):
        before = snapshot(); after = copy.deepcopy(before)
        method = after['source']['cri']['files'][0]['services'][0]['methods'][0]
        method['output'] = ['DifferentResponse']
        result = report.build_report(before, after)
        self.assertEqual(result['changes']['protobuf_rpcs'][0]['path'], '/cri/api.proto/Runtime/methods/Read/output/0')
        del after['source']['cri']['files'][0]['services'][0]['methods']
        result = report.build_report(before, after)
        self.assertTrue(any(c['kind'] == 'removed' for c in result['changes']['protobuf_rpcs']))

    def test_named_descriptors_align_by_name_instead_of_array_offset(self):
        before = snapshot(); after = copy.deepcopy(before)
        fields = before['source']['cri']['files'][0]['messages'][0]['fields']
        fields.append({'name': ['other'], 'number': [2], 'type': [9]})
        after = copy.deepcopy(before)
        after['source']['cri']['files'][0]['messages'][0]['fields'].pop(0)
        result = report.build_report(before, after)
        change = result['changes']['protobuf_fields'][0]
        self.assertEqual(change['kind'], 'removed')
        self.assertTrue(change['path'].endswith('/fields/field'))
        self.assertEqual(len(result['changes']['protobuf_fields']), 1)

    def test_removals_across_schema_defaults_and_gates_are_explicit(self):
        before = snapshot(); after = copy.deepcopy(before)
        del after['source']['openapi_definitions']['Example']
        del after['defaults']['cases']['zero']['kubelet']['nested']
        del after['apiserver']['apiserver_options']['flags']['secure-port']
        del after['defaults']['registered_feature_gates']['Gate']
        result = report.build_report(before, after)
        self.assertEqual(result['removal_count'], 4)
        self.assertTrue(all(c['kind'] == 'removed' for items in result['changes'].values() for c in items))

    def test_boolean_integer_and_missing_null_are_distinct(self):
        before = snapshot(); after = copy.deepcopy(before)
        after['defaults']['cases']['zero']['kubelet']['enabled'] = 0
        after['defaults']['cases']['zero']['kubelet']['absent'] = None
        result = report.build_report(before, after)
        self.assertEqual([c['kind'] for c in result['changes']['component_defaults']], ['added', 'type_changed'])
        self.assertEqual(result['change_count'], 2)

    def test_empty_named_lists_and_absent_rpc_domain_are_not_silently_equal(self):
        before = snapshot(); after = copy.deepcopy(before)
        after['source']['cri']['files'][0]['messages'][0]['fields'] = []
        result = report.build_report(before, after)
        self.assertEqual(result['changes']['protobuf_fields'][0]['kind'], 'removed')
        after = copy.deepcopy(before)
        after['source']['cri']['files'][0]['services'] = []
        del before['source']['cri']['files'][0]['services']
        result = report.build_report(before, after)
        self.assertEqual(result['changes']['protobuf_rpcs'][0]['kind'], 'added')
        after = copy.deepcopy(before)
        after['defaults']['apiserver_options'] = {'unexpected': True}
        result = report.build_report(before, after)
        self.assertEqual(result['changes']['snapshot_metadata'][0]['path'], '/defaults/apiserver_options')

    def test_source_pin_and_unknown_metadata_changes_are_never_dropped(self):
        before = snapshot(); after = copy.deepcopy(before)
        after['source']['authority']['commit'] = 'c' * 40
        after['apiserver']['future_metadata'] = {'value': 1}
        result = report.build_report(before, after)
        self.assertEqual(result['change_count'], 2)
        self.assertEqual(result['source_pins']['after']['source']['authority']['commit'], 'c' * 40)

    def test_missing_files_malformed_nonfinite_and_duplicate_json_fail_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            with self.assertRaises(OSError): report.load_snapshot(directory)
            for invalid in (b'{', b'{"schema_version":1,"schema_version":1}', b'{"value":NaN}'):
                write_snapshot(directory, snapshot())
                (directory / report.FILES['defaults']).write_bytes(invalid)
                with self.subTest(invalid=invalid), self.assertRaises(ValueError): report.load_snapshot(directory)

    def test_missing_domains_boolean_schema_version_and_duplicate_descriptor_fail(self):
        for mutation in ('domain', 'version', 'duplicate'):
            value = snapshot()
            if mutation == 'domain': del value['source']['cri']
            elif mutation == 'version': value['defaults']['schema_version'] = True
            else: value['source']['cri']['files'].append(copy.deepcopy(value['source']['cri']['files'][0]))
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): report.build_report(value, value)

    def test_truncated_gzip_and_wrong_descriptor_collection_type_are_invalid(self):
        value = snapshot()
        value['source']['cri']['files'][0]['services'] = {}
        with self.assertRaisesRegex(ValueError, 'descriptor array'):
            report.build_report(value, value)
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            write_snapshot(directory, snapshot())
            path = directory / 'inventory.json.gz'
            path.write_bytes(path.read_bytes()[:-8])
            result = subprocess.run([sys.executable, str(HERE / 'report.py'), '--before', str(directory),
                                     '--after', str(directory)], capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 2)

    def test_cli_exit_codes_and_both_formats_leave_snapshot_bytes_unchanged(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); before = root / 'before'; after = root / 'after'; before.mkdir(); after.mkdir()
            write_snapshot(before, snapshot()); write_snapshot(after, snapshot())
            command = [sys.executable, str(HERE / 'report.py'), '--before', str(before), '--after', str(after)]
            result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)['status'], 'unchanged')
            changed = snapshot(); changed['defaults']['cases']['zero']['kubelet']['enabled'] = True
            write_snapshot(after, changed)
            saved = {p: p.read_bytes() for p in root.rglob('*') if p.is_file()}
            result = subprocess.run(command + ['--format', 'markdown'], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn('Component defaults', result.stdout)
            self.assertIn('/defaults/zero/kubelet/enabled', result.stdout)
            self.assertEqual({p: p.read_bytes() for p in saved}, saved)
            (after / 'defaults.json').unlink()
            self.assertEqual(subprocess.run(command, capture_output=True, timeout=10).returncode, 2)

    def test_current_official_snapshots_have_no_drift(self):
        current = {'source': report.strict_json(gzip.decompress((HERE / 'inventory.json.gz').read_bytes())),
                   'defaults': report.strict_json((ROOT / 'tools/defaults/expected.json').read_bytes()),
                   'apiserver': report.strict_json((ROOT / 'tools/defaults/apiserver.expected.json').read_bytes())}
        result = report.build_report(current, copy.deepcopy(current))
        self.assertEqual(result['change_count'], 0)


if __name__ == '__main__': unittest.main()
