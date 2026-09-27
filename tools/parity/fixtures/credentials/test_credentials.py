import copy,hashlib,importlib.util,json,pathlib,subprocess,sys,tempfile,unittest
from unittest import mock
import verify,capture

COMPONENTS=('apiserver','kubelet')
SOURCE_FILES={'apiserver_capture_test.go','kubelet_capture_test.go','Capture.Dockerfile','capture.py'}
BASELINE_FILES={'go.mod','go.sum','pkg/kubernetes/apiserver/kubeconfig.go','pkg/kubernetes/apiserver/service_account.go','pkg/kubernetes/kubelet/kubeconfig.go'}
EVIDENCE_FILES={'build.log','receipt.json'}|{name+suffix for name in COMPONENTS for suffix in ('.json','-0.log','-1.log')}
def check_inventories(receipt,provenance):
 verify.equal(set(receipt['source_sha256']),SOURCE_FILES)
 verify.equal(set(receipt['outputs']),{name+'.json' for name in COMPONENTS})
 verify.equal(set(provenance['source_sha256']),BASELINE_FILES)
 verify.equal(set(provenance['fixture_sha256']),{'verify.py','test_credentials.py'}|{'evidence/'+name for name in EVIDENCE_FILES})
def check_repeats(root):
 for component in COMPONENTS:
  expected=verify.load(root/(component+'.json'));verify.equal(expected,verify.expected(component))
  for index in range(2):
   lines=(root/(component+'-'+str(index)+'.log')).read_text().splitlines()
   records=[line.removeprefix('RUBIX_CAPTURE ') for line in lines if line.startswith('RUBIX_CAPTURE ')]
   verify.equal(len(records),1)
   # Reuse the strict loader without permitting duplicate keys/nonfinite values.
   with tempfile.TemporaryDirectory() as temporary:
    path=pathlib.Path(temporary)/'record.json';path.write_text(records[0]);actual=verify.load(path)
   verify.equal(actual,expected)

class Credentials(unittest.TestCase):
 def test_source_expectations(self):
  for name in ('apiserver','kubelet'):verify.verify(verify.expected(name))
 def test_changed_identity_trust_permission_restart_and_crypto_fail(self):
  changes=[('apiserver',['checks','key_bits'],1024),('apiserver',['checks','key_mode'],'0644'),('apiserver',['checks','restart_preserves_key'],False),('apiserver',['checks','key_valid'],1),('apiserver',['synthetic_kubeconfig','current-context'],'admin-token@kubesolo'),('kubelet',['path_kubeconfig','clusters','kubernetes','certificate-authority'],'/wrong'),('kubelet',['checks','output_directory_fails'],False)]
  for component,path,value in changes:
   changed=verify.expected(component);target=changed
   for key in path[:-1]:target=target[key]
   target[path[-1]]=value
   with self.subTest(path=path),self.assertRaises(ValueError):verify.verify(changed)
 def test_missing_check_and_extra_credential_fail(self):
  value=verify.expected('apiserver');del value['checks']['missing_certificate_fails']
  with self.assertRaises(ValueError):verify.verify(value)
  value=verify.expected('apiserver');value['synthetic_kubeconfig']['users']['admin-token']['token']='secret'
  with self.assertRaises(ValueError):verify.verify(value)
 def test_strict_json(self):
  with tempfile.TemporaryDirectory() as d:
   path=pathlib.Path(d)/'test.json'
   for text in ('{"a":1,"a":2}','{"a":NaN}'):
    path.write_text(text)
    with self.assertRaises(ValueError):verify.load(path)
 def test_cleanup_errors_still_publish_receipt(self):
  with tempfile.TemporaryDirectory() as d:
   report={'containers':['owned'],'cleanup_errors':[]}
   with mock.patch.object(capture.helper.subprocess,'run',side_effect=OSError('unavailable')):capture.helper.finish(report,pathlib.Path(d),'owned')
   result=verify.load(pathlib.Path(d)/'receipt.json')
   self.assertEqual(len(result['cleanup_errors']),4);self.assertIsNone(result['remaining_containers'])
 def test_optimized_cli_rejects_mutation(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)
   for name in ('apiserver','kubelet'):(root/(name+'.json')).write_text(json.dumps(verify.expected(name)))
   command=[sys.executable,'-O',str(pathlib.Path(__file__).parent/'verify.py'),str(root)]
   self.assertEqual(subprocess.run(command,capture_output=True).returncode,0)
   (root/'kubelet.json').write_text(json.dumps(verify.expected('apiserver')))
   self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
   (root/'apiserver.json').write_text(json.dumps(verify.expected('kubelet')))
   self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
   (root/'kubelet.json').write_text(json.dumps(verify.expected('kubelet')))
   changed=verify.expected('apiserver');changed['checks']['key_valid']=1
   (root/'apiserver.json').write_text(json.dumps(changed))
   self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
 def test_durable_capture(self):
  for component in ('apiserver','kubelet'):verify.verify(verify.load(pathlib.Path(__file__).parent/'evidence'/(component+'.json')))
 def test_receipt_and_provenance_bind_inputs_and_outputs(self):
  here=pathlib.Path(__file__).parent
  receipt=verify.load(here/'evidence/receipt.json');provenance=verify.load(here/'provenance.json')
  check_inventories(receipt,provenance);check_repeats(here/'evidence')
  self.assertEqual(receipt['helper_sha256'],hashlib.sha256((capture.ROOT/'tools/defaults/capture.py').read_bytes()).hexdigest())
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True)
  for name,digest in receipt['source_sha256'].items():self.assertEqual(hashlib.sha256((here/name).read_bytes()).hexdigest(),digest)
  for name,digest in receipt['outputs'].items():self.assertEqual(hashlib.sha256((here/'evidence'/name).read_bytes()).hexdigest(),digest)
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(hashlib.sha256((here/name).read_bytes()).hexdigest(),digest)
 def test_inventory_omissions_fail(self):
  here=pathlib.Path(__file__).parent
  receipt=verify.load(here/'evidence/receipt.json');provenance=verify.load(here/'provenance.json')
  for owner,key in [('receipt','source_sha256'),('receipt','outputs'),('provenance','source_sha256'),('provenance','fixture_sha256')]:
   r=copy.deepcopy(receipt);p=copy.deepcopy(provenance);mapping=(r if owner=='receipt' else p)[key]
   del mapping[next(iter(mapping))]
   with self.subTest(owner=owner,key=key),self.assertRaises(ValueError):check_inventories(r,p)
 def test_missing_duplicate_and_changed_repeat_records_fail(self):
  import shutil
  source=pathlib.Path(__file__).parent/'evidence'
  for mutation in ('missing','duplicate','changed'):
   with tempfile.TemporaryDirectory() as temporary:
    root=pathlib.Path(temporary)
    for name in EVIDENCE_FILES:shutil.copyfile(source/name,root/name)
    path=root/'apiserver-1.log';lines=path.read_text().splitlines();record=next(line for line in lines if line.startswith('RUBIX_CAPTURE '))
    if mutation=='missing':lines.remove(record)
    elif mutation=='duplicate':lines.append(record)
    else:
     value=json.loads(record.removeprefix('RUBIX_CAPTURE '));value['checks']['key_valid']=1
     lines[lines.index(record)]='RUBIX_CAPTURE '+json.dumps(value)
    path.write_text('\n'.join(lines)+'\n')
    with self.subTest(mutation=mutation),self.assertRaises(ValueError):check_repeats(root)
if __name__=='__main__':unittest.main()
