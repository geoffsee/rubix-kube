import copy
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import capture
import verify


def require_engineering_budget(measurements):
    """Acceptance of captured elapsed samples, distinct from diagnostic watchdogs."""
    if not measurements:
        raise ValueError('missing measured elapsed samples')
    for elapsed in measurements:
        if type(elapsed) is not int or not 0 <= elapsed <= 35000:
            raise ValueError('measured shutdown exceeds 35000 ms engineering gate')


class EvidenceTests(unittest.TestCase):

    def test_frozen_elapsed_samples_meet_engineering_gate(self):
        verify.verify()
        lines=self.log.decode().splitlines()
        samples=[int(line.rsplit('elapsed_ms=',1)[1]) for line in lines if line.startswith('RUBIX_OWNED_SIGNAL ')]
        self.assertEqual(len(samples),61)
        require_engineering_budget(samples)

    def test_engineering_gate_rejects_one_millisecond_overrun(self):
        require_engineering_budget([35000])
        with self.assertRaisesRegex(ValueError, '35000 ms engineering gate'):
            require_engineering_budget([35001])

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

    def test_owned_process_mutations(self):
        for old,new in [(b'combined_cases=61',b'combined_cases=60'),(b'owner_joined=true',b'owner_joined=false'),(b'dependent_started=false',b'dependent_started=true'),(b'sentinel_survived=true',b'sentinel_survived=false'),(b'case=force iteration=20',b'case=full iteration=20'),(b'RUBIX_NAMESPACE ',b'INVALID_NAMESPACE ')]:
            changed=self.log.replace(old,new,1)
            self.assertNotEqual(changed,self.log)
            with self.assertRaises(ValueError):verify.validate_owned(changed.decode().splitlines())

    def test_namespace_identities_must_be_distinct_positive_processes(self):
        lines=self.log.decode().splitlines()
        index=next(i for i,line in enumerate(lines) if line.startswith('RUBIX_NAMESPACE '))
        import json
        for shell,helper,processes in [(2,2,[1,2,2]),(-2,3,[1,-2,3]),(2,0,[1,2,0]),(2,3,[1,2,2])]:
            changed=list(lines)
            changed[index]='RUBIX_NAMESPACE '+json.dumps(dict(init=1,shell=shell,helper=helper,processes=processes))
            with self.assertRaises(ValueError):verify.validate_owned(changed)

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

    def test_metadata_failures_leave_no_capture_directory_or_docker_activity(self):
        import subprocess
        import sys
        for failure in [subprocess.TimeoutExpired(['git','rev-parse','HEAD'],30), subprocess.CalledProcessError(1,['git','rev-parse','HEAD']), OSError('git unavailable')]:
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as temporary:
                output=Path(temporary)/'capture'
                with patch.object(sys,'argv',['capture.py','--output',str(output)]), patch.object(capture.subprocess,'check_output',side_effect=failure) as metadata, patch.object(capture.helper,'bounded') as docker, patch.object(capture.helper,'finish') as cleanup:
                    with self.assertRaises(type(failure)):capture.main()
                metadata.assert_called_once_with(['git','rev-parse','HEAD'],cwd=capture.ROOT,text=True,timeout=30)
                docker.assert_not_called();cleanup.assert_not_called()
                self.assertFalse(output.exists())

    def test_helper_digest_failure_precedes_output_creation(self):
        import sys
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)/'capture'
            with patch.object(sys,'argv',['capture.py','--output',str(output)]), patch.object(capture.subprocess,'check_output',return_value='a'*40+'\n'), patch.object(capture,'digest',side_effect=OSError('helper unreadable')), patch.object(capture.helper,'bounded') as docker, patch.object(capture.helper,'finish') as cleanup:
                with self.assertRaises(OSError):capture.main()
            docker.assert_not_called();cleanup.assert_not_called()
            self.assertFalse(output.exists())

    def test_existing_capture_is_not_overwritten(self):
        import sys
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)/'capture';output.mkdir()
            (output/'receipt.json').write_text('historical failure')
            with patch.object(sys,'argv',['capture.py','--output',str(output)]), patch.object(capture.subprocess,'check_output',return_value='a'*40+'\n'), patch.object(capture.helper,'bounded') as docker, patch.object(capture.helper,'finish') as cleanup:
                with self.assertRaises(FileExistsError):capture.main()
            docker.assert_not_called();cleanup.assert_not_called()
            self.assertEqual((output/'receipt.json').read_text(),'historical failure')

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
