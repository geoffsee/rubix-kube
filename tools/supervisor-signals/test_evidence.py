import copy
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import capture
import verify

class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.receipt = verify.loads(verify.read(verify.HERE / 'evidence/receipt.json'))
        self.log = verify.read(verify.HERE / 'evidence/run.log')

    def test_frozen_evidence_and_current_compiled_inputs(self):
        verify.verify()

    def test_exact_receipt_inventory_and_types(self):
        for key in self.receipt:
            changed = copy.deepcopy(self.receipt)
            del changed[key]
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify.validate_receipt(changed, self.log)
        for key, value in [('schema_version', True), ('owned_subprocesses_reaped', 1), ('signal_cases', 159), ('cleanup_errors', ['failed']), ('remaining_containers', ['survivor'])]:
            changed = copy.deepcopy(self.receipt)
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify.validate_receipt(changed, self.log)

    def test_raw_claim_mutations_fail_even_when_rehashed(self):
        for old, new in [(b'cases=160', b'cases=159'), (b'all_children_reaped=true', b'all_children_reaped=false'), (b'1 passed;', b'0 passed;'), (self.receipt['test_binary_sha256'].encode(), b'0' * 64)]:
            log = self.log.replace(old, new)
            receipt = copy.deepcopy(self.receipt)
            receipt['run_sha256'] = hashlib.sha256(log).hexdigest()
            with self.subTest(old=old), self.assertRaises(ValueError):
                verify.validate_receipt(receipt, log)

    def test_parser_rejects_duplicate_and_nonfinite_values(self):
        for value in ['{"a":1,"a":2}', 'NaN', 'Infinity', '1e400', '-1e400']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                verify.loads(value)

    def test_current_lock_and_supervisor_changes_require_recapture(self):
        original = verify.digest
        for suffix in ['Cargo.lock', 'rust-toolchain.toml', 'crates/rubix-supervisor/src/signals.rs', 'Cargo.toml']:
            target = verify.ROOT / suffix
            with self.subTest(suffix=suffix), patch.object(verify, 'digest', side_effect=lambda path: '0' * 64 if path == target else original(path)), self.assertRaises(ValueError):
                verify.verify()

    def test_missing_historical_source_cannot_evade_current_binding(self):
        original = verify.loads
        def without_coordinator(data):
            value = original(data)
            if type(value) is dict and 'crates/rubix-supervisor/src/coordinator.rs' in value:
                del value['crates/rubix-supervisor/src/coordinator.rs']
            return value
        with patch.object(verify, 'loads', side_effect=without_coordinator), self.assertRaisesRegex(ValueError, 'compiled input inventory'):
            verify.verify()

    def test_cleanup_failure_still_records_all_attempts(self):
        report = {'containers': ['owned-test'], 'cleanup_errors': []}
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(capture.helper.subprocess, 'run', side_effect=OSError('daemon unavailable')) as run:
                capture.helper.finish(report, Path(temporary), 'owned-image')
            self.assertEqual(run.call_count, 4)
            recorded = verify.loads((Path(temporary) / 'receipt.json').read_bytes())
            self.assertEqual(len(recorded['cleanup_errors']), 4)
            self.assertIsNone(recorded['remaining_containers'])
            self.assertIsNone(recorded['remaining_images'])

if __name__ == '__main__':
    unittest.main()
