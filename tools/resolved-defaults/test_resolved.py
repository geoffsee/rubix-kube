import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import capture
import verify

HERE=Path(__file__).resolve().parent
class Resolved(unittest.TestCase):
 def setUp(self):self.value=verify.load(HERE/'expected.json')
 def test_frozen_source_semantics(self):verify.verify(self.value)
 def test_resolution_mutations_fail(self):
  mutations=[('default','service_ip','10.0.0.2'),('dual_stack','secondary_service_cidr','fd00::/108'),('default','advertise_address','0.0.0.0'),('default','anonymous_auth',True),('dual_stack','authorization_modes',['AlwaysAllow']),('dual_stack','events_history_window','1m15s'),('dual_stack','runtime_config',{'api/v1':'true','apps/v1':'true'}),('dual_stack','watch_cache_sizes',['pods#42']),('dual_stack','token_max_expiration','0s'),('default','listener_created',True),('default','generated_serving_certificate',1)]
  for case,key,new in mutations:
   with self.subTest(case=case,key=key):
    value=copy.deepcopy(self.value);value['cases'][case][key]=new
    with self.assertRaises(ValueError):verify.verify(value)
 def test_error_cases_cannot_disappear_or_succeed(self):
  for name in ('invalid_cidr','small_cidr','invalid_watch_cache','invalid_token_expiration'):
   value=copy.deepcopy(self.value);value['cases'][name]={'error':''}
   with self.assertRaises(ValueError):verify.verify(value)
   del value['cases'][name]
   with self.assertRaises(ValueError):verify.verify(value)
 def test_flags_before_and_after_are_observed(self):
  value=copy.deepcopy(self.value);value['cases']['default']['flags_after']=copy.deepcopy(value['cases']['default']['flags_before'])
  with self.assertRaises(ValueError):verify.verify(value)
 def test_strict_json(self):
  with tempfile.TemporaryDirectory() as d:
   path=Path(d)/'input.json'
   for text in ('{"a":0,"a":1}','{"a":NaN}'):
    path.write_text(text)
    with self.assertRaises(ValueError):verify.load(path)
 def test_optimized_cli_rejects_missing_type_and_unreviewed_flags(self):
  command=[sys.executable,'-O',str(HERE/'verify.py')]
  self.assertEqual(subprocess.run(command+[str(HERE/'expected.json')],capture_output=True).returncode,0)
  with tempfile.TemporaryDirectory() as d:
   path=Path(d)/'input.json'
   for kind in ('empty','bool_as_int','unreviewed_flag'):
    value=copy.deepcopy(self.value)
    if kind=='empty':value={}
    elif kind=='bool_as_int':value['controls']['server_started']=0
    else:value['cases']['default']['flags_after']['request-timeout']='61s'
    path.write_text(json.dumps(value))
    self.assertNotEqual(subprocess.run(command+[str(path)],capture_output=True).returncode,0,kind)
 def test_complete_comparison_preserves_absence_and_types(self):
  for a,b in [({}, {'a':None}),({'a':0},{'a':False}),({'a':[]},{'a':{}})]:
   with self.assertRaises(ValueError):verify.equal(a,b)
 def test_current_sources_and_receipt_are_bound(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json')
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True)
  self.assertEqual(receipt['inputs'],verify.load(HERE/'inputs.json'))
  self.assertEqual(receipt['helper_sha256'],capture.digest(capture.HELPER))
  self.assertEqual(set(receipt['source_sha256']),{'main.go','Dockerfile','capture.py','inputs.json'})
  for name,digest in receipt['source_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
  for name,digest in receipt['output_sha256'].items():self.assertEqual(capture.digest(HERE/'evidence'/name),digest)
  for name,digest in provenance['files_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
  self.assertEqual(capture.digest(HERE/'expected.json'),provenance['expected_sha256'])
 def test_two_live_records_equal_frozen(self):
  for index in range(2):verify.equal(verify.load(HERE/'evidence'/f'run{index}.json'),self.value)
 def test_frozen_identity_cannot_be_silently_refreshed(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);(root/'expected.json').write_text(json.dumps(self.value))
   (root/'provenance.json').write_text(json.dumps({'expected_sha256':'0'*64}))
   with mock.patch.object(verify,'HERE',root),mock.patch.object(sys,'argv',['verify',str(root/'expected.json')]):
    with self.assertRaisesRegex(ValueError,'identity mismatch'):verify.main()
 def test_bad_prepared_source_fails_before_docker_or_output(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);bad=root/'bad';bad.write_bytes(b'bad')
   with mock.patch.object(sys,'argv',['capture','--source-archive',str(bad),'--go-archive',str(bad),'--output',str(root/'out')]),mock.patch.object(capture.helper,'bounded') as run:
    with self.assertRaisesRegex(ValueError,'identity mismatch'):capture.main()
    run.assert_not_called();self.assertFalse((root/'out').exists())
 def test_cleanup_failures_still_publish_receipt(self):
  with tempfile.TemporaryDirectory() as d:
   report={'containers':['owned'],'cleanup_errors':[]}
   with mock.patch.object(capture.helper.subprocess,'run',side_effect=OSError('unavailable')):capture.helper.finish(report,Path(d),'owned')
   result=verify.load(Path(d)/'receipt.json');self.assertEqual(len(result['cleanup_errors']),4);self.assertIsNone(result['remaining_containers'])
if __name__=='__main__':unittest.main()
