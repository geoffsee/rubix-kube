import copy,hashlib,json,pathlib,subprocess,sys,tempfile,unittest
from unittest import mock
import capture,verify
HERE=pathlib.Path(__file__).resolve().parent
SOURCE_FILES={'webhook_capture_test.go','Capture.Dockerfile','capture.py'}
BASELINE_FILES={'go.mod','go.sum'}|{'pkg/kubernetes/webhook/'+name+'.go' for name in ('config','executor','loadbalancer','loadbalancer_test','patch','service','webhooks')}
EVIDENCE_FILES={'build.log','receipt.json','webhook.json','webhook-0.log','webhook-1.log'}
def check_inventories(receipt,provenance):
 verify.equal(set(receipt['source_sha256']),SOURCE_FILES)
 verify.equal(set(receipt['outputs']),{'webhook.json'})
 verify.equal(set(provenance['source_sha256']),BASELINE_FILES)
 verify.equal(set(provenance['fixture_sha256']),{'verify.py','test_webhooks.py'}|{'evidence/'+name for name in EVIDENCE_FILES})
def check_repeats(root):
 expected=verify.load(root/'webhook.json');verify.verify(expected)
 for index in range(2):
  records=[line.removeprefix('RUBIX_CAPTURE ') for line in (root/f'webhook-{index}.log').read_text().splitlines() if line.startswith('RUBIX_CAPTURE ')]
  verify.equal(len(records),1)
  with tempfile.TemporaryDirectory() as d:
   path=pathlib.Path(d)/'record.json';path.write_text(records[0]);actual=verify.load(path)
  verify.equal(actual,expected)
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
  check_inventories(receipt,provenance);check_repeats(HERE/'evidence')
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True)
  self.assertEqual(receipt['helper_sha256'],capture.digest(capture.ROOT/'tools/defaults/capture.py'))
  for name,digest in receipt['source_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
  for name,digest in receipt['outputs'].items():self.assertEqual(capture.digest(HERE/'evidence'/name),digest)
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
 def test_inventory_omissions_fail(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json')
  for owner,key in [('receipt','source_sha256'),('receipt','outputs'),('provenance','source_sha256'),('provenance','fixture_sha256')]:
   r=copy.deepcopy(receipt);p=copy.deepcopy(provenance);mapping=(r if owner=='receipt' else p)[key];del mapping[next(iter(mapping))]
   with self.subTest(owner=owner,key=key),self.assertRaises(ValueError):check_inventories(r,p)
 def test_repeat_record_omission_duplication_and_mutation_fail(self):
  import shutil
  for mutation in ('missing','duplicate','changed'):
   with tempfile.TemporaryDirectory() as d:
    root=pathlib.Path(d)
    for name in EVIDENCE_FILES:shutil.copyfile(HERE/'evidence'/name,root/name)
    path=root/'webhook-1.log';lines=path.read_text().splitlines();record=next(line for line in lines if line.startswith('RUBIX_CAPTURE '))
    if mutation=='missing':lines.remove(record)
    elif mutation=='duplicate':lines.append(record)
    else:
     value=json.loads(record.removeprefix('RUBIX_CAPTURE '));value['handler_requests']['service_dry_run']['scheduled_status_update']=1
     lines[lines.index(record)]='RUBIX_CAPTURE '+json.dumps(value)
    path.write_text('\n'.join(lines)+'\n')
    with self.subTest(mutation=mutation),self.assertRaises(ValueError):check_repeats(root)
if __name__=='__main__':unittest.main()
