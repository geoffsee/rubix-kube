import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import verify

class EvidenceTests(unittest.TestCase):
    def test_frozen_evidence(self):
        verify.verify()
    def test_strict_json_rejects_duplicates_nonfinite(self):
        for raw in [b'{"a":1,"a":2}',b'NaN',b'1e999']:
            with self.assertRaises(ValueError): verify.loads(raw)
    def test_markers_reject_omission_duplicate_wrong_family_and_order(self):
        raw=verify.read(verify.HERE/'evidence/run0.log')
        for changed in [raw.replace(b'RUBIX_PREFLIGHT_PORTS ipv4_2379 conflict_then_rebind',b'wrong'),
                        raw+ b'\nRUBIX_PREFLIGHT_FILES exact pass\n',
                        raw.replace(b'ipv6_6443',b'ipv4_6443'),
                        raw.replace(b'RUBIX_PREFLIGHT_FILES permission_denied pass',b'RUBIX_PREFLIGHT_FILES permission_denied success')]:
            with self.assertRaises(ValueError): verify.markers(changed)
    def changed_receipt(self,key,value):
        original=verify.loads
        def altered(raw):
            result=original(raw)
            if isinstance(result,dict) and result.get('schema_version')==1 and 'runs' in result:
                result=copy.deepcopy(result);result[key]=value
            return result
        return patch.object(verify,'loads',side_effect=altered)
    def test_cleanup_platform_and_boolean_schema_mutations_fail(self):
        for key,value in [('remaining_containers',['owned']),('cleanup_errors',['failure']),
                          ('schema_version',True),('platform','linux/amd64'),('uncommitted_implementation',1)]:
            with self.changed_receipt(key,value),self.assertRaises(ValueError):verify.verify()
    def test_omitted_run_and_foreign_owned_identity_fail(self):
        report=verify.loads(verify.read(verify.HERE/'evidence/receipt.json'))
        for key,value in [('runs',{'run0.log':report['runs']['run0.log']}),('containers',['foreign-0','foreign-1'])]:
            with self.changed_receipt(key,value),self.assertRaises(ValueError):verify.verify()
    def test_missing_historical_module_cannot_evade_current_binding(self):
        original=verify.loads
        def altered(raw):
            result=original(raw)
            if isinstance(result,dict) and 'crates/rubix-platform/src/preflight_probe.rs' in result:
                result.pop('crates/rubix-platform/src/preflight_probe.rs')
            return result
        with patch.object(verify,'loads',side_effect=altered),self.assertRaisesRegex(ValueError,'compiled input inventory'):
            verify.verify()
    def test_current_source_tamper_fails(self):
        original=verify.digest
        def altered(path):
            if str(path).endswith('crates/rubix-platform/src/preflight_probe.rs'):return '0'*64
            return original(path)
        with patch.object(verify,'digest',side_effect=altered),self.assertRaisesRegex(ValueError,'compiled source changed'):
            verify.verify()
    def test_tampered_raw_and_source_inventory_rejected(self):
        import shutil
        for name in ['run0.log','source-hashes.json']:
            with tempfile.TemporaryDirectory() as temporary:
                here=Path(temporary)/'fixture';shutil.copytree(verify.HERE,here)
                path=here/'evidence'/name;path.write_bytes(path.read_bytes()+b' ')
                with self.assertRaisesRegex(ValueError,'evidence digest'):verify.verify(here)
    def test_missing_cleanup_field_rejected(self):
        original=verify.loads
        def altered(raw):
            result=original(raw)
            if isinstance(result,dict) and 'cleanup_errors' in result:result.pop('cleanup_errors')
            return result
        with patch.object(verify,'loads',side_effect=altered),self.assertRaisesRegex(ValueError,'receipt inventory'):verify.verify()
    def test_symlink_evidence_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary);(path/'real').write_bytes(b'{}');(path/'link').symlink_to('real')
            with self.assertRaises(ValueError):verify.read(path/'link')


class CaptureTests(unittest.TestCase):
    def test_revision_failure_leaves_output_retryable(self):
        import capture
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)/'capture'
            with patch('sys.argv',['capture','--output',str(output)]),patch.object(capture,'short',side_effect=RuntimeError('metadata failure')):
                with self.assertRaises(RuntimeError):capture.main()
            self.assertFalse(output.exists())
    def test_helper_digest_failure_leaves_output_retryable(self):
        import capture
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)/'capture'
            with patch('sys.argv',['capture','--output',str(output)]),patch.object(capture,'short',return_value='0'*40),patch.object(capture,'digest',side_effect=OSError('unreadable helper')):
                with self.assertRaises(OSError):capture.main()
            self.assertFalse(output.exists())

if __name__=='__main__':unittest.main()
