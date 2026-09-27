import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import tempfile
from unittest import mock

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('resource_verify', HERE / 'verify.py')
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


class ResourceOracleTests(unittest.TestCase):
    def setUp(self):
        self.records = json.loads((HERE / 'expected.json').read_text())

    def test_real_go_capture_satisfies_independent_semantics(self):
        verifier.verify(self.records)

    def test_changed_storage_reclaim_policy_is_rejected(self):
        record = next(r for r in self.records if r['component'] == 'localpath')
        record['objects']['storageclasses'][0]['reclaimPolicy'] = 'Delete'
        with self.assertRaises(AssertionError):
            verifier.verify(self.records)

    def test_broken_service_selector_is_rejected(self):
        self.records[0]['objects']['services'][0]['spec']['selector']['k8s-app'] = 'wrong'
        with self.assertRaises(AssertionError):
            verifier.verify(self.records)

    def test_changed_dns_forwarding_is_rejected(self):
        r = next(r for r in self.records if r['component'] == 'coredns' and r['variant'] == 'container-dual')
        r['objects']['configmaps'][0]['data']['Corefile'] = 'invalid'
        with self.assertRaises(AssertionError):
            verifier.verify(self.records)

    def test_unknown_variant_is_rejected(self):
        self.records[0]['variant'] = 'unqualified-variant'
        with self.assertRaises(AssertionError):
            verifier.verify(self.records)

    def test_missing_named_bootstrap_check_is_rejected(self):
        record = next(r for r in self.records if r['component'] == 'portainer')
        del record['checks']['bootstrap_existing_secret_preserved']
        with self.assertRaises(AssertionError):
            verifier.verify(self.records)

    def test_daemon_failure_still_publishes_unknown_cleanup_receipt(self):
        spec = importlib.util.spec_from_file_location('resource_capture', HERE / 'capture.py')
        capture = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(capture)
        with tempfile.TemporaryDirectory() as temporary:
            report = {'cleanup_errors': []}
            with mock.patch.object(capture, 'run', side_effect=OSError('daemon unavailable')), mock.patch.object(capture.subprocess, 'check_output', side_effect=OSError('daemon unavailable')):
                capture.cleanup_and_receipt(report, 'owned-tag', ['owned-container'], Path(temporary))
            saved = json.loads((Path(temporary) / 'receipt.json').read_text())
            self.assertEqual(len(saved['cleanup_errors']), 4)
            self.assertIsNone(saved['remaining_containers'])
            self.assertIsNone(saved['remaining_images'])

    def test_durable_file_hashes_match_receipt(self):
        manifest = json.loads((HERE / 'provenance.json').read_text())
        for path, digest in manifest['durable_sha256'].items():
            self.assertEqual(hashlib.sha256((HERE.parent / path).read_bytes()).hexdigest(), digest, path)


if __name__ == '__main__':
    unittest.main()
