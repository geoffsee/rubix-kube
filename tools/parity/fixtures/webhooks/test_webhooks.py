import copy,hashlib,json,pathlib,subprocess,sys,tempfile,unittest
from unittest import mock
import capture,verify
HERE=pathlib.Path(__file__).resolve().parent
class Webhooks(unittest.TestCase):
 def test_captured_output_matches_complete_independent_expectations(self):verify.verify(verify.load(HERE/'evidence/webhook.json'))
 def test_registration_security_and_patch_mutations_fail(self):
  mutations=[(['configurations','true','webhooks',0,'sideEffects'],'None'),(['configurations','true','webhooks',0,'failurePolicy'],'Fail'),(['configurations','true','webhooks',0,'rules',0,'operations'],['CREATE','UPDATE']),(['configurations','true','webhooks',0,'clientConfig','caBundle'],'changed'),(['handler_requests','pod_unassigned','admission_review','response','uid'],'wrong'),(['handler_requests','pod_unassigned','admission_review','response','allowed'],1),(['handler_requests','service_dry_run','scheduled_status_update'],True),(['fake_client_status','assign','actions',1,'subresource'],''),(['fake_client_status','patch_failure','patch_count'],1)]
  for path,value in mutations:
   changed=verify.expected();target=changed
   for key in path[:-1]:target=target[key]
   target[path[-1]]=value
   with self.subTest(path=path),self.assertRaises(ValueError):verify.verify(changed)
 def test_every_service_case_checks_scheduled_state(self):
  for name in ('service_dry_run','service_disabled','service_no_address','service_clusterip'):
   value=verify.expected();case=value['handler_requests'][name]
   self.assertEqual(case['admission_review']['request']['object']['metadata'],{'name':'svc','namespace':'default'})
   case['scheduled_status_update']=True
   with self.subTest(name=name),self.assertRaises(ValueError):verify.verify(value)
 def test_missing_family_and_weakness_rewrite_fail(self):
  for path in [['fake_client_status'],['handler_requests','malformed_typed_object'],['handler_requests','pvc_existing_annotation']]:
   value=verify.expected();target=value
   for key in path[:-1]:target=target[key]
   del target[path[-1]]
   with self.assertRaises(ValueError):verify.verify(value)
 def test_optimized_cli_checks_errors_types_and_inventory(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d);path=root/'webhook.json';path.write_text(json.dumps(verify.expected()));command=[sys.executable,'-O',str(HERE/'verify.py'),str(root)]
   self.assertEqual(subprocess.run(command,capture_output=True).returncode,0)
   for value in [{},{'component':'apiserver'},dict(verify.expected(),component='other')]:
    path.write_text(json.dumps(value));self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
 def test_strict_json(self):
  with tempfile.TemporaryDirectory() as d:
   path=pathlib.Path(d)/'bad'
   for raw in ['{"a":1,"a":2}','{"a":NaN}','{"a":Infinity}']:
    path.write_text(raw)
    with self.assertRaises(ValueError):verify.load(path)
 def test_cleanup_failure_still_reports(self):
  with tempfile.TemporaryDirectory() as d:
   report={'containers':['owned'],'cleanup_errors':[]}
   with mock.patch.object(capture.helper.subprocess,'run',side_effect=OSError('unavailable')):capture.helper.finish(report,pathlib.Path(d),'owned')
   result=verify.load(pathlib.Path(d)/'receipt.json');self.assertEqual(len(result['cleanup_errors']),4);self.assertIsNone(result['remaining_containers'])
 def test_receipt_and_provenance(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json')
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True)
  self.assertEqual(receipt['helper_sha256'],capture.digest(capture.ROOT/'tools/defaults/capture.py'))
  for name,digest in receipt['source_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
  for name,digest in receipt['outputs'].items():self.assertEqual(capture.digest(HERE/'evidence'/name),digest)
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
if __name__=='__main__':unittest.main()
