import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('resolved_report', HERE / 'report.py')
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


class ResolvedReport(unittest.TestCase):
    def setUp(self):
        self.record = json.loads((HERE / 'expected.json').read_text())
        self.receipt = json.loads((HERE / 'evidence/receipt.json').read_text())
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.before = self.write('before', self.record)

    def write(self, name, record, receipt=None):
        directory = self.root / name
        directory.mkdir(exist_ok=True)
        raw = (json.dumps(record, sort_keys=True) + '\n').encode()
        (directory / 'resolved.json').write_bytes(raw)
        receipt = copy.deepcopy(receipt if receipt is not None else self.receipt)
        digest = hashlib.sha256(raw).hexdigest()
        receipt['output_sha256'] = {'run0.json': digest, 'run1.json': digest}
        (directory / 'receipt.json').write_text(json.dumps(receipt))
        return directory

    def compare(self, record, receipt=None):
        after = self.write('after', record, receipt)
        return report.build_report(report.load_snapshot(self.before), report.load_snapshot(after))

    def test_unchanged_and_receipt_run_identity_do_not_create_behavior_changes(self):
        receipt = copy.deepcopy(self.receipt)
        receipt['containers'] = ['different-disposable-run']
        result = self.compare(self.record, receipt)
        self.assertEqual(result['status'], 'unchanged')
        self.assertEqual(result['change_count'], 0)
        self.assertNotEqual(result['snapshot_sha256']['before']['receipt.json'],
                            result['snapshot_sha256']['after']['receipt.json'])

    def test_resolved_change_does_not_obscure_constructor_inventory(self):
        value = copy.deepcopy(self.record)
        value['cases']['default']['service_ip'] = '10.0.0.2'
        result = self.compare(value)
        self.assertEqual(result['changes']['resolved_apiserver_options'],
                         [{'path': '/default/service_ip', 'kind': 'changed',
                           'before': '10.0.0.1', 'after': '10.0.0.2'}])
        self.assertEqual(result['changes']['resolved_snapshot_metadata'], [])
        self.assertNotIn('apiserver_defaults', result['changes'])

    def test_flag_changes_and_removals_are_preserved(self):
        value = copy.deepcopy(self.record)
        value['cases']['default']['flags_after']['request-timeout'] = '61s'
        del value['cases']['default']['flags_before']['request-timeout']
        result = self.compare(value)
        self.assertEqual(result['change_count'], 2)
        self.assertEqual(result['removal_count'], 1)

    def test_error_text_and_success_error_transition_are_separate(self):
        value = copy.deepcopy(self.record)
        value['cases']['invalid_cidr']['error'] = 'new upstream error'
        value['cases']['default'] = {'error': 'completion now rejects options'}
        result = self.compare(value)
        self.assertEqual(result['change_count'], 3)
        self.assertEqual(result['removal_count'], 1)
        errors = result['changes']['resolved_completion_errors']
        self.assertEqual({c['kind'] for c in errors}, {'added', 'changed'})
        reverse = report.build_report(report.load_snapshot(self.root / 'after'),
                                      report.load_snapshot(self.before))
        self.assertEqual(reverse['changes']['resolved_apiserver_options'][0]['kind'], 'added')

    def test_case_removal_and_addition_use_names(self):
        value = copy.deepcopy(self.record)
        value['cases']['renamed'] = value['cases'].pop('invalid_cidr')
        result = self.compare(value)
        self.assertEqual(result['change_count'], 2)
        self.assertEqual(result['removal_count'], 1)

    def test_absence_null_bool_int_and_unknown_fields_never_compare_equal(self):
        for old, new in [(False, 0), (None, ''), ([], {}), (1, 1.0)]:
            value = copy.deepcopy(self.record)
            value['cases']['default']['new_field'] = old
            before = self.write('typed-before', value)
            value['cases']['default']['new_field'] = new
            after = self.write('typed-after', value)
            result = report.build_report(report.load_snapshot(before), report.load_snapshot(after))
            self.assertEqual(result['changes']['resolved_apiserver_options'][0]['kind'], 'type_changed')
        value = copy.deepcopy(self.record)
        value['cases']['default']['new_field'] = None
        self.assertEqual(self.compare(value)['changes']['resolved_apiserver_options'][0]['kind'], 'added')
        value = copy.deepcopy(self.record)
        value['extension'] = {'optional': None}
        self.assertEqual(self.compare(value)['changes']['resolved_snapshot_metadata'][0]['kind'], 'added')

    def test_source_pin_and_controls_changes_are_metadata(self):
        receipt = copy.deepcopy(self.receipt)
        receipt['inputs']['source']['revision'] = 'a' * 40
        value = copy.deepcopy(self.record)
        value['controls']['external_address'] = '192.0.2.3'
        result = self.compare(value, receipt)
        self.assertEqual(len(result['changes']['resolved_snapshot_metadata']), 2)
        self.assertEqual(result['changes']['resolved_apiserver_options'], [])
        self.assertEqual(result['source_pins']['after']['inputs']['source']['revision'], 'a' * 40)

    def test_missing_mixed_and_malformed_shape_fail(self):
        mutations = [lambda x: x.update(schema_version=True),
                     lambda x: x['controls'].pop('operation'),
                     lambda x: x['controls'].update(server_started=0),
                     lambda x: x['cases']['default'].update(error='mixed'),
                     lambda x: x['cases']['invalid_cidr'].update(error=None),
                     lambda x: x['cases']['default'].pop('flags_before'),
                     lambda x: x['runtime'].update(go='unbound'),
                     lambda x: x.update(cases={})]
        for mutate in mutations:
            value = copy.deepcopy(self.record)
            mutate(value)
            with self.assertRaises(ValueError):
                report.load_snapshot(self.write('invalid', value))

    def test_receipt_failure_and_identity_type_negatives(self):
        mutations = [lambda x: x.update(identical_repeats=1),
                     lambda x: x.update(errors=['failed']),
                     lambda x: x.update(cleanup_errors=['leaked container']),
                     lambda x: x.pop('remaining_images'),
                     lambda x: x.update(source_sha256={}),
                     lambda x: x['inputs']['source'].update(bytes=True),
                     lambda x: x['inputs']['source'].update(revision='moving-tag'),
                     lambda x: x.update(helper_sha256='')]
        for mutate in mutations:
            receipt = copy.deepcopy(self.receipt)
            mutate(receipt)
            with self.assertRaises(ValueError):
                report.load_snapshot(self.write('invalid-receipt', self.record, receipt))

    def test_wrong_digest_or_missing_snapshot_fails_without_refresh(self):
        before = (self.before / 'resolved.json').read_bytes()
        (self.before / 'resolved.json').write_bytes(before + b' ')
        with self.assertRaises(ValueError):
            report.load_snapshot(self.before)
        (self.before / 'resolved.json').unlink()
        with self.assertRaises(OSError):
            report.load_snapshot(self.before)

    def test_strict_json_and_input_bounds(self):
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":Infinity}', b'{"a":1e999}'):
            with self.assertRaises(ValueError):
                report.strict_json(raw)
        with mock.patch.object(report, 'LIMIT', 8):
            with self.assertRaises(ValueError):
                report.load_snapshot(self.before)

    def test_cli_exit_codes_optimized_and_deterministic_markdown(self):
        command = [sys.executable, '-O', str(HERE / 'report.py'), '--before', str(self.before)]
        unchanged = subprocess.run(command + ['--after', str(self.before)], capture_output=True)
        self.assertEqual(unchanged.returncode, 0, unchanged.stderr)
        value = copy.deepcopy(self.record)
        value['cases']['default']['service_ip'] = '10.0.0.2'
        after = self.write('cli-after', value)
        raw_before = (after / 'resolved.json').read_bytes()
        changed = subprocess.run(command + ['--after', str(after), '--format', 'markdown'], capture_output=True)
        self.assertEqual(changed.returncode, 1, changed.stderr)
        repeated = subprocess.run(command + ['--after', str(after), '--format', 'markdown'], capture_output=True)
        self.assertEqual(changed.stdout, repeated.stdout)
        self.assertEqual((after / 'resolved.json').read_bytes(), raw_before)
        self.assertIn(b'Resolved apiserver options', changed.stdout)
        (after / 'resolved.json').write_text('{}')
        self.assertEqual(subprocess.run(command + ['--after', str(after)], capture_output=True).returncode, 2)


if __name__ == '__main__':
    unittest.main()
