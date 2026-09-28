"""Read-only evidence regressions plus mocked owned-resource cleanup."""
import copy, importlib.util, json, tempfile, unittest
from pathlib import Path
from unittest.mock import Mock, patch
HERE=Path(__file__).resolve().parent
def module(name):
 spec=importlib.util.spec_from_file_location('alpine_'+name,HERE/(name+'.py'));result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
v=module('verify');capture=module('capture')
class EvidenceTests(unittest.TestCase):
 def test_duplicate_and_nonfinite_json(self):
  for raw in ['{"a":1,"a":2}','NaN','1e999']:
   with self.assertRaises(ValueError):v.loads(raw)
 def test_bool_is_not_integer(self):
  with self.assertRaises(ValueError):v.equal({'install':False},{'install':0},'typed mismatch')
 def test_reaped_group_is_never_signaled_and_files_retained(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);output=root/'output';output.mkdir();private=root/'private';private.mkdir()
   vm=Mock(pid=12345,returncode=0);vm.poll.return_value=0
   report={'errors':[],'status':'passed'}
   with patch.object(capture.os,'killpg') as kill:
    capture.finish(report,output,vm,None,private,None,None,None,None,[])
   self.assertEqual(kill.call_args_list,[((12345,0),)])
   self.assertTrue(private.exists());self.assertFalse(report['owned_temporary_directory_removed'])
   self.assertEqual(report['status'],'failed')
   self.assertEqual(json.loads((output/'result.json').read_text())['status'],'failed')
 def test_absent_reaped_group_removes_owned_files(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);output=root/'output';output.mkdir();private=root/'private';private.mkdir()
   vm=Mock(pid=12345,returncode=0);vm.poll.return_value=0
   report={'errors':[],'status':'passed'}
   with patch.object(capture.os,'killpg',side_effect=ProcessLookupError):
    capture.finish(report,output,vm,None,private,None,None,None,None,[])
   self.assertFalse(private.exists());self.assertTrue(report['owned_process_group_absent'])
 def test_private_diagnostic_is_suppressed(self):
  with tempfile.TemporaryDirectory() as tmp:
   output=Path(tmp);(output/'console.log').write_bytes(b'opaque-private-value')
   report={'errors':[],'status':'passed'}
   capture.finish(report,output,None,None,None,None,None,None,None,[b'opaque-private-value'])
   self.assertNotIn(b'opaque-private-value',(output/'console.log').read_bytes())
   self.assertEqual(report['status'],'failed')
   self.assertEqual(json.loads((output/'result.json').read_text())['status'],'failed')
 def test_current_evidence(self):v.verify()
 def test_raw_log_digest_tamper(self):
  original=v.read
  def altered(path):
   raw=original(path)
   return raw+b'changed' if path.name=='baseline-inventory.stdout' else raw
  with patch.object(v,'read',side_effect=altered):
   with self.assertRaisesRegex(ValueError,'raw evidence digest'):v.verify()
 def test_no_reboot_rejected(self):
  original=v.read
  def altered(path):
   return original(path.parent/'before-reboot-id.stdout') if path.name=='reboot-readiness.stdout' else original(path)
  with patch.object(v,'read',side_effect=altered):
   with self.assertRaisesRegex(ValueError,'actual fresh kernel boot'):v.verify_guest(HERE/'evidence/first')

 def mutate_semantic(self,old,new):
  before=(HERE/'evidence/first/baseline-inventory.stdout').read_text();reboot=(HERE/'evidence/first/reboot-verification.stdout').read_text()
  self.assertIn(old,before)
  with self.assertRaises(ValueError):v.semantic(before.replace(old,new),reboot)
 def test_missing_controller(self):self.mutate_semantic('CONTROLLERS_after cpuset cpu io memory hugetlb pids dmem','CONTROLLERS_after cpu io memory hugetlb pids dmem')
 def test_missing_no_opt_in_evidence(self):self.mutate_semantic('RUBIX_NO_OPT_IN_UNCHANGED','')
 def test_changed_baseline_error(self):self.mutate_semantic(v.NETWORK_ERROR,'different failure')
 def test_wrong_package_delta(self):self.mutate_semantic('nftables-1.1.6-r1','nftables-1.1.7-r0')
 def test_raw_boolean_mutation(self):self.mutate_semantic('"install":false','"install":0')
 def test_guest_missing_cleanup(self):
  self.mutate_report(lambda r:r.pop('owned_process_group_absent'))
 def test_guest_missing_source(self):self.mutate_report(lambda r:r['source_sha256'].pop('capture.py'))
 def test_extra_host_drive(self):self.mutate_report(lambda r:r['qemu_argv'].extend(['-drive','file=/host/example,format=raw']))
 def test_guest_network_not_restricted(self):self.mutate_report(lambda r:r.update(qemu_argv=[s.replace('restrict=on','restrict=off') for s in r['qemu_argv']]))
 def mutate_report(self,mutate):
  original=v.read
  report=v.loads(original(HERE/'evidence/first/result.json'));mutate(report)
  def altered(path):return json.dumps(report).encode() if path.name=='result.json' else original(path)
  with patch.object(v,'read',side_effect=altered):
   with self.assertRaises(ValueError):v.verify_guest(HERE/'evidence/first')
if __name__=='__main__':unittest.main()
