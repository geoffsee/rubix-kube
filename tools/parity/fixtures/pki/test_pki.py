import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('pki_verify', HERE / 'verify.py')
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


class PKIOracleTests(unittest.TestCase):
    def setUp(self):
        self.records = json.loads((HERE / 'expected.json').read_text())

    def test_optimized_cli_rejects_empty_and_mutated_records(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'capture.json'
            self.records[0]['fresh']['ca']['key_mode'] = '0644'
            for records in ([], self.records):
                source.write_text(json.dumps(records))
                result = subprocess.run([sys.executable, '-O', str(HERE / 'verify.py'), str(source)], capture_output=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)

    def test_ipv6_rotation_is_distinct_from_invalid_sans(self):
        self.records[0]['ipv6_extra_san_rotates'] = True
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_integer_cannot_replace_certificate_boolean(self):
        self.records[0]['fresh']['ca']['is_ca'] = 1
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_boolean_cannot_replace_certificate_number(self):
        self.records[0]['fresh']['ca']['key_usage'] = True
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_real_crypto_capture_meets_policy(self):
        verifier.verify(self.records)

    def test_replaced_trust_root_is_rejected(self):
        self.records[0]['rotations']['node-ip-change']['stable_certificates']['ca'] = False
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_mismatched_key_is_rejected_for_healthy_capture(self):
        self.records[0]['fresh']['admin']['key_matches'] = False
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_missing_extra_san_is_rejected(self):
        self.records[0]['fresh']['apiserver']['dns'].remove('api.fixture.test')
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_missing_standard_api_dns_is_rejected(self):
        self.records[0]['fresh']['apiserver']['dns'].remove('kubernetes.default')
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_missing_kubelet_san_is_rejected(self):
        self.records[0]['fresh']['kubelet']['dns'].remove('fixture-node')
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_changed_extended_usage_is_rejected(self):
        self.records[0]['fresh']['admin']['extended_key_usage'] = [1]
        with self.assertRaises(ValueError):
            verifier.verify(self.records)

    def test_capture_exports_no_private_key_material(self):
        for path in [HERE / 'expected.json', HERE / 'evidence/pki.log']:
            self.assertNotIn('PRIVATE KEY-----', path.read_text())

    def test_weakened_key_permissions_are_rejected(self):
        self.records[0]['fresh']['ca']['key_mode'] = '0644'
        with self.assertRaises(ValueError):
            verifier.verify(self.records)


if __name__ == '__main__':
    unittest.main()
