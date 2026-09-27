import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent

def load(name):
    spec = importlib.util.spec_from_file_location('config_api_' + name, HERE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

verify = load('verify').verify

class ConfigAPITests(unittest.TestCase):
    def setUp(self):
        self.file = json.loads((HERE / 'config.json').read_text())
        self.api = json.loads((HERE / 'configapi.json').read_text())

    def test_real_baseline_matches_independent_semantics(self):
        verify(self.file, self.api)

    def test_numeric_boolean_substitution_is_rejected(self):
        r = self.api[0]['requests']['get']
        r['body']['config']['logging']['debug'] = 0
        r['raw_body'] = json.dumps(r['body'])
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_raw_numeric_boolean_cannot_match_recorded_false(self):
        response = self.api[0]['requests']['get']
        original = response['raw_body']
        response['raw_body'] = original.replace('"debug":false', '"debug":0')
        self.assertNotEqual(response['raw_body'], original)
        self.assertIs(response['body']['config']['logging']['debug'], False)
        with self.assertRaises(AssertionError):
            verify(self.file, self.api)

    def test_failed_write_corruption_is_rejected(self):
        self.file[0]['failed_write_preserves_original'] = False
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_stale_etag_acceptance_is_rejected(self):
        self.api[0]['requests']['patch-stale-etag']['status'] = 200
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_secret_leak_in_default_response_is_rejected(self):
        r = self.api[0]['requests']['get']
        r['body']['config']['portainer']['edgeKey'] = 'fixture-synthetic-key'
        r['raw_body'] = json.dumps(r['body'])
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_lost_concurrent_update_is_rejected(self):
        r = self.api[0]['requests']['after-concurrent-patches']
        r['body']['config']['logging']['debug'] = False
        r['raw_body'] = json.dumps(r['body'])
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_missing_socket_cleanup_is_rejected(self):
        self.api[0]['checks']['shutdown_removes_socket'] = False
        with self.assertRaises(AssertionError): verify(self.file, self.api)

    def test_daemon_failure_still_publishes_cleanup_receipt(self):
        capture = load('capture')
        with tempfile.TemporaryDirectory() as directory:
            report = {'cleanup_errors': []}
            with mock.patch.object(capture, 'run', side_effect=OSError('offline')), mock.patch.object(capture.subprocess, 'check_output', side_effect=OSError('offline')):
                capture.cleanup_and_receipt(report, 'owned', ['owned-container'], Path(directory))
            result = json.loads((Path(directory) / 'receipt.json').read_text())
            self.assertEqual(len(result['cleanup_errors']), 4)
            self.assertIsNone(result['remaining_containers'])
            self.assertIsNone(result['remaining_images'])

    def test_provenance_matches_durable_files(self):
        for path, digest in json.loads((HERE / 'provenance.json').read_text())['durable_sha256'].items():
            self.assertEqual(hashlib.sha256((HERE / path).read_bytes()).hexdigest(), digest, path)

if __name__ == '__main__': unittest.main()
