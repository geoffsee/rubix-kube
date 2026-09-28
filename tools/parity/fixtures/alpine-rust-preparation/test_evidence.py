"""No VM boot: strict receipt and independent semantic mutation regressions."""
import contextlib,copy,io,importlib.util,json,tempfile,unittest,shutil
from pathlib import Path
from unittest.mock import patch
HERE=Path(__file__).resolve().parent
def module(name):
 spec=importlib.util.spec_from_file_location('rust_alpine_'+name,HERE/(name+'.py'));result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
v=module('verify');capture=module('capture')
class BootstrapTests(unittest.TestCase):
 def test_missing_input_cache_fails_before_artifact_metadata_or_output(self):
  with tempfile.TemporaryDirectory() as temporary:
   root=Path(temporary);output=root/'output'
   args=['capture','--allow-privileged-vm','--output',str(output),'--image-cache',str(root/'cache'),'--artifact-directory',str(root/'artifact')]
   with patch('sys.argv',args),patch.object(capture,'validate_artifact') as artifact,patch.object(capture.subprocess,'check_output') as metadata,contextlib.redirect_stderr(io.StringIO()):
    with self.assertRaises(SystemExit) as result:capture.main()
   self.assertEqual(result.exception.code,2);artifact.assert_not_called();metadata.assert_not_called()
   self.assertFalse(output.exists());self.assertFalse((root/'cache').exists())
 def cloud(self):
  return {'status':'done','errors':[],'recoverable_errors':{'WARNING':[v.WARNING]},**{stage:{'errors':[]} for stage in ['init-local','init','modules-config','modules-final']}}
 def test_capture_rejects_unexplained_degradation_and_stage_failures(self):
  for code,cloud in [(2,{**self.cloud(),'recoverable_errors':{}}),(True,self.cloud()),(1,self.cloud()),(2,{**self.cloud(),'recoverable_errors':{'WARNING':['other']}}),(0,{**self.cloud(),'modules-final':{'errors':['failed']}})]:
   with self.assertRaises(ValueError):capture.baseline.validate_cloud_init(code,cloud)
  capture.baseline.validate_cloud_init(2,self.cloud())
 def test_both_boots_require_raw_identity_and_exact_warning(self):
  with tempfile.TemporaryDirectory() as temporary:
   directory=Path(temporary)
   report={}
   for label,key in [('cloud-init','cloud_init'),('reboot-cloud-init','reboot_cloud_init')]:
    report[key]=self.cloud();report[key+'_exit']=2
    (directory/(label+'.stdout')).write_text(json.dumps(report[key]))
   v.verify_bootstrap(report,directory)
   for label,key in [('cloud-init','cloud_init'),('reboot-cloud-init','reboot_cloud_init')]:
    for edit in ['empty_warning','unknown_warning','failed_stage','raw_drift','missing_raw','duplicate_raw','nonfinite_raw']:
     original=(directory/(label+'.stdout')).read_bytes();changed=copy.deepcopy(report)
     if edit in ['empty_warning','unknown_warning','failed_stage']:
      if edit=='empty_warning':changed[key]['recoverable_errors']={}
      elif edit=='unknown_warning':changed[key]['recoverable_errors']={'WARNING':['other']}
      else:changed[key]['init']['errors']=['failed']
      (directory/(label+'.stdout')).write_text(json.dumps(changed[key]))
     elif edit=='raw_drift':changed[key]['status']='running'
     elif edit=='missing_raw':(directory/(label+'.stdout')).unlink()
     elif edit=='duplicate_raw':(directory/(label+'.stdout')).write_text('{"status":"done","status":"done"}')
     else:(directory/(label+'.stdout')).write_text('{"bad":1e999}')
     with self.subTest(stage=key,mutation=edit),self.assertRaises((ValueError,FileNotFoundError)):v.verify_bootstrap(changed,directory)
     (directory/(label+'.stdout')).write_bytes(original)
 def test_public_key_rejects_host_account_comments(self):
  v.verify_public_key('ssh-ed25519 AAAA rubix-alpine-fixture-host')
  for value in ['ssh-ed25519 AAAA local-user@local-host','ssh-ed25519 AAAA','ssh-ed25519 AAAA rubix-alpine-fixture-client',None]:
   with self.assertRaises(ValueError):v.verify_public_key(value)
 def test_key_generation_uses_fixture_only_comments(self):
  for name in ['host','client']:
   args=capture.baseline.keygen_arguments(name,Path('/owned')/name)
   self.assertEqual(args,['ssh-keygen','-q','-t','ed25519','-N','','-C','rubix-alpine-fixture-'+name,'-f','/owned/'+name])

class EvidenceTests(unittest.TestCase):
 def test_current_evidence(self):v.verify()
 def test_dangling_case_marker(self):
  with self.assertRaises(ValueError):v.cases("CASE_unmatched_BEGIN\n")
 def test_duplicate_nonfinite(self):
  for raw in ['{"a":1,"a":2}','NaN','1e999']:
   with self.assertRaises(ValueError):v.loads(raw)
 def test_bool_integer_distinct(self):
  with self.assertRaises(ValueError):v.equal(False,0,'type')
 def semantic_mutation(self,old,new):
  before=(HERE/'evidence/first/baseline-inventory.stdout').read_text();reboot=(HERE/'evidence/first/reboot-verification.stdout').read_text();self.assertIn(old,before)
  with self.assertRaises(ValueError):v.semantic(before.replace(old,new),reboot)
 def test_no_effect_success_rejected(self):self.semantic_mutation('CASE_injected_no_effect_service_BEGIN\nEXIT 1','CASE_injected_no_effect_service_BEGIN\nEXIT 0')
 def test_missing_recheck(self):self.semantic_mutation('prerequisite is still missing after preparation','ignored missing state')
 def test_missing_controller(self):self.semantic_mutation('CONTROLLERS_after cpuset cpu io memory hugetlb pids dmem','CONTROLLERS_after cpu io memory hugetlb pids dmem')
 def test_partial_effects_not_omitted(self):self.semantic_mutation('host state may have changed','state unchanged')
 def test_wrong_package_effect(self):self.semantic_mutation('nftables-1.1.6-r1','nftables-1.1.7-r0')
 def test_failed_restore(self):self.semantic_mutation('SERVICE_RESTORED ','SERVICE_NOT_RESTORED ')
 def test_opt_out_invariance_missing(self):self.semantic_mutation('NO_OPT_IN_UNCHANGED','NO_EVIDENCE')
 def mutate_report(self,change):
  original=v.read;report=v.loads(original(HERE/'evidence/first/result.json'));change(report)
  def altered(path):return json.dumps(report).encode() if path.name=='result.json' else original(path)
  with patch.object(v,'read',side_effect=altered):
   with self.assertRaises(ValueError):v.verify_guest(HERE/'evidence/first')
 def test_missing_cleanup(self):self.mutate_report(lambda r:r.pop('owned_process_group_absent'))
 def test_helper_drift(self):self.mutate_report(lambda r:r.update(baseline_helper_sha256='0'*64))
 def test_wrong_artifact(self):self.mutate_report(lambda r:r.update(artifact_sha256='0'*64))
 def test_extra_host_device(self):self.mutate_report(lambda r:r['qemu_argv'].extend(['-drive','file=/host/example']))
 def test_raw_digest_tamper(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);shutil.copytree(HERE/'evidence',root/'evidence');shutil.copyfile(HERE/'provenance.json',root/'provenance.json')
   path=root/'evidence/first/baseline-inventory.stdout';path.write_bytes(path.read_bytes()+b'changed')
   with self.assertRaisesRegex(ValueError,'raw evidence digest'):v.verify(root)
 def test_missing_build_revision_rejected_before_output(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);binary=root/'rubixctl';binary.write_bytes(b'x'*5352968)
   pin=json.loads((HERE/'inputs.json').read_text())['artifact'];metadata=dict(pin);metadata['revision']='0'*40
   (root/'artifact.json').write_text(json.dumps(metadata));(root/'receipt.json').write_text(json.dumps({'revision':pin['revision']}))
   with patch.object(capture.subprocess,'run'),patch.object(capture,'digest',return_value=pin['sha256']):
    with self.assertRaisesRegex(ValueError,'revision mismatch'):capture.validate_artifact(root)
if __name__=='__main__':unittest.main()
