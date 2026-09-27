import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import verify
import capture

HERE=Path(__file__).resolve().parent

class DefaultTests(unittest.TestCase):
    def setUp(self):
        self.value=json.loads((HERE/'expected.json').read_text())

    def test_selected_expected_fixture_is_hash_bound(self):
        self.assertEqual(verify.load_expected(HERE/'expected.json'),self.value)
        changed=copy.deepcopy(self.value)
        name=next(name for name in changed['registered_feature_gates'] if name not in ('RotateKubeletServerCertificate','SidecarContainers'))
        changed['registered_feature_gates'][name]['enabled']=not changed['registered_feature_gates'][name]['enabled']
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'changed.json';path.write_text(json.dumps(changed))
            result=subprocess.run([sys.executable,str(HERE/'verify.py'),str(path),'--expected',str(path)],capture_output=True,text=True,timeout=10)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('expected fixture provenance mismatch',result.stderr)

    def test_selected_expected_fixture_is_semantically_checked(self):
        self.value['cases']['zero']['kubelet']['authentication']['anonymous']['enabled']=True
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/'changed.json';path.write_text(json.dumps(self.value))
            with self.assertRaisesRegex(ValueError,'anonymous authentication'):verify.load_expected(path)

    def test_real_official_output_satisfies_independent_assertions(self):
        verify.verify(self.value)

    def test_omitted_generated_cloud_defaults_are_rejected(self):
        self.value['cases']['zero']['controller']['KubeCloudShared']['NodeMonitorPeriod']='0s'
        with self.assertRaisesRegex(ValueError,'nested cloud'):verify.verify(self.value)

    def test_missing_reserved_memory_generated_rounding_is_rejected(self):
        self.value['cases']['explicit']['kubelet']['reservedMemory'][0]['limits']['memory']='1000100u'
        with self.assertRaisesRegex(ValueError,'ResourceList'):verify.verify(self.value)

    def test_nested_explicit_false_is_not_overwritten(self):
        self.value['cases']['explicit']['controller']['KubeCloudShared']['ConfigureCloudRoutes']=True
        with self.assertRaisesRegex(ValueError,'explicit cloud'):verify.verify(self.value)

    def test_changed_default_is_rejected(self):
        self.value['cases']['zero']['kubelet']['authentication']['anonymous']['enabled']=True
        with self.assertRaises(ValueError):verify.verify(self.value)

    def test_explicit_false_cannot_be_redefaulted(self):
        self.value['cases']['explicit']['kubelet']['enableServer']=True
        with self.assertRaises(ValueError):verify.verify(self.value)

    def test_changed_feature_gate_default_is_rejected(self):
        self.value['registered_feature_gates']['RotateKubeletServerCertificate']['enabled']=False
        with self.assertRaises(ValueError):verify.verify(self.value)

    def test_gate_removal_is_a_reviewable_difference(self):
        altered=copy.deepcopy(self.value)
        del altered['registered_feature_gates']['SidecarContainers']
        changes=list(verify.differences(self.value,altered))
        self.assertEqual(changes[0]['path'],'/registered_feature_gates/SidecarContainers')
        self.assertIn('removed',changes[0])

    def test_boolean_to_integer_mutation_is_not_equal(self):
        changed=copy.deepcopy(self.value)
        name=next(iter(changed['registered_feature_gates']))
        entry=changed['registered_feature_gates'][name]['specs'][0]
        entry['locked']=int(entry['locked'])
        changes=list(verify.differences(self.value,changed))
        self.assertEqual(len(changes),1)
        self.assertTrue(changes[0]['path'].endswith('/specs/0/locked'))

    def test_duplicate_or_nonfinite_json_is_rejected(self):
        for raw in ('{"a":0,"a":1}','{"a":NaN}','{"a":Infinity}'):
            with self.subTest(raw=raw),self.assertRaises(ValueError):verify.load_json(raw)

    def test_output_limit_is_enforced(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError,'output limit'):
                capture.bounded(['python3','-c','print("x"*4096)'],Path(directory)/'out',10,64)
            self.assertLessEqual((Path(directory)/'out').stat().st_size,64)

    def test_cleanup_failures_do_not_skip_other_cleanup_or_receipt(self):
        report={'containers':['owned-a','owned-b'],'cleanup_errors':[]}
        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.object(capture.subprocess,'run',side_effect=OSError('daemon unavailable')) as run:
                capture.finish(report,Path(directory),'owned-image')
            self.assertEqual(run.call_count,5)
            result=json.loads((Path(directory)/'receipt.json').read_text())
            self.assertEqual(len(result['cleanup_errors']),5)
            self.assertIsNone(result['remaining_containers'])
            self.assertIsNone(result['remaining_images'])

    def test_durable_receipt_binds_executed_sources_and_output(self):
        receipt=json.loads((HERE/'evidence/receipt.json').read_text())
        self.assertEqual(receipt['errors'],[])
        self.assertEqual(receipt['cleanup_errors'],[])
        self.assertEqual(receipt['remaining_containers'],[])
        self.assertEqual(receipt['remaining_images'],[])
        self.assertTrue(receipt['identical_repeats'])
        for name,digest in receipt['source_sha256'].items():
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)
        self.assertEqual(hashlib.sha256((HERE/'expected.json').read_bytes()).hexdigest(),receipt['output_sha256'])

    def test_archive_pin_agrees_with_accepted_toolchain(self):
        accepted=json.loads((HERE.parents[1]/'docs/architecture/upstream-inputs.json').read_text())
        inputs=json.loads((HERE/'inputs.json').read_text())
        pin=next(a for a in accepted['go_extraction_toolchain']['archives'] if a['os']=='linux' and a['arch']=='arm64')
        self.assertEqual(inputs['go']['sha256'],pin['sha256'])
        self.assertEqual(inputs['go']['bytes'],pin['size'])

if __name__=='__main__':unittest.main()
