import copy,hashlib,importlib.util,json,pathlib,subprocess,sys,tempfile,unittest
from unittest import mock
import verify,capture
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
  self.assertEqual(receipt['helper_sha256'],hashlib.sha256((capture.ROOT/'tools/defaults/capture.py').read_bytes()).hexdigest())
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True)
  for name,digest in receipt['source_sha256'].items():self.assertEqual(hashlib.sha256((here/name).read_bytes()).hexdigest(),digest)
  for name,digest in receipt['outputs'].items():self.assertEqual(hashlib.sha256((here/'evidence'/name).read_bytes()).hexdigest(),digest)
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(hashlib.sha256((here/name).read_bytes()).hexdigest(),digest)
if __name__=='__main__':unittest.main()
