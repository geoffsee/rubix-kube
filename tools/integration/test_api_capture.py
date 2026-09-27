"""Fresh-capture provenance, complete equality and raw evidence must all hold."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import check_api_capture as checker


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name)
        self.documents = {'fixtures.json': checker.load(checker.API / 'fixtures.json'),
                          'result.json': checker.load(checker.API / 'evidence/result.json'),
                          'runner-result.json': checker.load(checker.API / 'evidence/runner-result.json')}
        self.save()

    def save(self):
        for name, value in self.documents.items():
            (self.path / name).write_text(json.dumps(value))

    def test_reviewed_capture_passes_all_checks(self):
        result = checker.check(self.path)
        self.assertEqual(result['status'], 'verified')
        self.assertEqual(result['resource_documents'], 6)
        self.assertEqual(result['watch_events'], 3)

    def test_reviewed_fixture_must_match_its_provenance_digest(self):
        original = checker.load
        def altered(path):
            value = original(path)
            if path == checker.API / 'provenance.json':
                value['durable_sha256']['fixtures.json'] = '0' * 64
            return value
        with mock.patch.object(checker, 'load', side_effect=altered):
            with self.assertRaisesRegex(ValueError, 'frozen fixture hash'): checker.check(self.path)

    def test_full_comparison_rejects_non_anchor_field_changes(self):
        for name in ('pod-create', 'pod-read'):
            self.documents['fixtures.json']['cases'][name]['spec']['schedulerName'] = 'unreviewed'
        self.save()
        with self.assertRaisesRegex(ValueError, 'complete fixture'): checker.check(self.path)

    def test_current_runner_and_component_hashes_are_both_required(self):
        for document in ('result.json', 'runner-result.json'):
            original = copy.deepcopy(self.documents[document])
            self.documents[document]['source_sha256']['spike.py'] = '0' * 64
            self.save()
            with self.subTest(document=document), self.assertRaisesRegex(ValueError, 'source hashes'):
                checker.check(self.path)
            self.documents[document] = original

    def test_runtime_pins_and_recorded_fixture_cannot_diverge(self):
        self.documents['result.json']['inputs']['kubernetes_version'] = 'v0.0.0'
        self.save()
        with self.assertRaisesRegex(ValueError, 'input pins'): checker.check(self.path)

    def test_raw_numeric_boolean_substitution_is_not_equal(self):
        record = next(r for r in self.documents['result.json']['http'] if r['name'] == 'custom-read')
        raw = json.loads(record['raw_response']); raw['spec']['unknown']['bool'] = 0
        record['raw_response'] = json.dumps(raw)
        self.save()
        with self.assertRaisesRegex(ValueError, 'raw HTTP'): checker.check(self.path)

    def test_consistent_raw_mutation_still_cannot_change_the_reviewed_fixture(self):
        record = next(r for r in self.documents['result.json']['http'] if r['name'] == 'pod-read')
        record['response']['spec']['schedulerName'] = 'different'
        record['raw_response'] = json.dumps(record['response'])
        self.save()
        with self.assertRaisesRegex(ValueError, 'raw resource'): checker.check(self.path)

    def test_watch_raw_bytes_are_required_independently(self):
        self.documents['result.json']['raw_watch_lines'][0] = json.dumps({'type': 'DELETED', 'object': {}})
        self.save()
        with self.assertRaisesRegex(ValueError, 'raw watch'): checker.check(self.path)

    def test_shutdown_and_tls_failures_are_not_success(self):
        mutations = [('shutdown', lambda r: r['shutdowns'][0].update(forced=True)),
                     ('tls', lambda r: r['datastore_tls'][0].update(exit_code=0)),
                     ('unrelated TLS error', lambda r: r['datastore_tls'][0].update(diagnostic='bad option')),
                     ('missing TLS', lambda r: r.pop('datastore_tls')),
                     ('boolean exit', lambda r: r['shutdowns'][0].update(exit_code=False))]
        original = copy.deepcopy(self.documents['result.json'])
        for label, mutation in mutations:
            self.documents['result.json'] = copy.deepcopy(original)
            mutation(self.documents['result.json']); self.save()
            with self.subTest(label=label), self.assertRaises(ValueError): checker.check(self.path)

    def test_failed_or_incomplete_runner_is_rejected(self):
        for patch in ({'exit_code': 1}, {'exit_code': False}, {'errors': ['cleanup failed']}, {'errors': None}):
            original = copy.deepcopy(self.documents['runner-result.json'])
            self.documents['runner-result.json'].update(patch); self.save()
            with self.subTest(patch=patch), self.assertRaises(ValueError): checker.check(self.path)
            self.documents['runner-result.json'] = original

    def test_duplicate_nonfinite_and_missing_json_are_rejected(self):
        for raw in ('{"x":1,"x":2}', '{"x":NaN}', '{'):
            with self.subTest(raw=raw), self.assertRaises(ValueError): checker.strict_json(raw)
        (self.path / 'result.json').unlink()
        with self.assertRaises(OSError): checker.check(self.path)

    def test_optimized_cli_checks_and_failure_output_does_not_echo_raw_data(self):
        secret_marker = 'synthetic-do-not-echo-marker'
        self.documents['runner-result.json']['errors'] = [secret_marker]
        self.save()
        result = subprocess.run([sys.executable, '-O', str(Path(checker.__file__)), str(self.path)],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertNotIn(secret_marker, result.stdout + result.stderr)
        self.assertIn('validation failed', result.stderr)


if __name__ == '__main__': unittest.main()
