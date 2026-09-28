import copy,json,pathlib,shutil,sys,tempfile,unittest
from unittest.mock import patch
import qualify,verify_linux
from verify import digest,load,read,strict

class LinuxEvidence(unittest.TestCase):
 def test_frozen_real_executable_evidence_and_four_file_inventory(self):
  here=verify_linux.HERE;verify_linux.verify(here/'evidence-linux')
  provenance=load(here/'linux-provenance.json')
  self.assertEqual(set(provenance),{'schema','files'});self.assertEqual(provenance['schema'],1)
  self.assertEqual(set(provenance['files']),{'build.log','run.log','receipt.json','source-hashes.json'})
  for name,value in provenance['files'].items():self.assertEqual(digest(here/'evidence-linux'/name),value)
 def test_false_success_and_lost_ownership_claims_are_rejected(self):
  original=read(verify_linux.HERE/'evidence-linux/run.log')
  line=next(line for line in original.splitlines() if line.startswith(b'RUBIX_CHECK '))
  value=strict(line[12:]);mutations=[]
  for key in ['listener_survived','ports_released','files_unchanged']:
   changed=copy.deepcopy(value);changed[key]=False;mutations.append(changed)
  changed=copy.deepcopy(value);changed['cases'][6]['exit']=0;mutations.append(changed)
  changed=copy.deepcopy(value);changed['cases'][6]['stderr']='All 7 checks passed';mutations.append(changed)
  changed=copy.deepcopy(value);changed['cases'][0]['exit']=False;mutations.append(changed)
  changed=copy.deepcopy(value);changed['cases'].pop();mutations.append(changed)
  for changed in mutations:
   raw=original.replace(line,b'RUBIX_CHECK '+json.dumps(changed).encode())
   with self.assertRaises(ValueError):verify_linux.verify_run(raw)
 def test_exact_binary_inventory_and_completion_are_required(self):
  raw=read(verify_linux.HERE/'evidence-linux/run.log')
  for changed in [raw.replace(b'/out/check-tests',b'/out/rubixctl'),raw.replace(b'test result: ok. 7 passed;',b'test result: FAILED;')]:
   with self.assertRaises(ValueError):verify_linux.verify_run(changed)
 def test_compiled_lock_source_and_owned_command_are_bound(self):
  with tempfile.TemporaryDirectory() as tmp:
   dst=pathlib.Path(tmp)/'capture';shutil.copytree(verify_linux.HERE/'evidence-linux',dst)
   original=load(dst/'receipt.json')
   for key,value in [('image_id','unknown'),('remaining_containers',None),('extra',True)]:
    changed=copy.deepcopy(original);changed[key]=value;(dst/'receipt.json').write_text(json.dumps(changed))
    with self.assertRaises(ValueError):verify_linux.verify(dst)
   changed=copy.deepcopy(original);changed['command'].remove('--network');(dst/'receipt.json').write_text(json.dumps(changed))
   with self.assertRaises(ValueError):verify_linux.verify(dst)
   (dst/'receipt.json').write_text(json.dumps(original))
   real_digest=verify_linux.digest
   def drift(path):return '0'*64 if pathlib.Path(path)==verify_linux.ROOT/'Cargo.lock' else real_digest(path)
   with patch.object(verify_linux,'digest',side_effect=drift):
    with self.assertRaisesRegex(ValueError,'compiled source Cargo.lock'):verify_linux.verify(dst)
 def test_metadata_failure_precedes_output_creation(self):
  with tempfile.TemporaryDirectory() as tmp:
   output=pathlib.Path(tmp)/'capture'
   with patch.object(sys,'argv',['qualify.py','--output',str(output)]),patch.object(qualify.helper,'bounded',side_effect=RuntimeError('metadata')):
    with self.assertRaises(RuntimeError):qualify.main()
   self.assertFalse(output.exists())
 def test_harness_digest_failure_precedes_output_creation_or_docker(self):
  with tempfile.TemporaryDirectory() as tmp:
   output=pathlib.Path(tmp)/'capture';real_digest=qualify.digest
   def broken(path):
    if pathlib.Path(path)==qualify.HERE/'Linux.Dockerfile':raise OSError('unreadable harness')
    return real_digest(path)
   with patch.object(sys,'argv',['qualify.py','--output',str(output)]),patch.object(qualify,'digest',side_effect=broken),patch.object(qualify.helper,'bounded') as bounded:
    with self.assertRaises(OSError):qualify.main()
   self.assertFalse(output.exists());bounded.assert_not_called()
 def test_build_failure_still_reports_every_cleanup_failure(self):
  with tempfile.TemporaryDirectory() as tmp:
   output=pathlib.Path(tmp)/'capture'
   def bounded(command,path,*_):
    if command[0]=='git':pathlib.Path(path).write_text('a'*40+'\n')
    else:raise RuntimeError('build failure')
   with patch.object(sys,'argv',['qualify.py','--output',str(output)]),patch.object(qualify.helper,'bounded',side_effect=bounded),patch.object(qualify.helper.subprocess,'run',side_effect=RuntimeError('daemon unavailable')) as cleanup:
    self.assertEqual(qualify.main(),1)
   receipt=load(output/'receipt.json');self.assertEqual(cleanup.call_count,3);self.assertEqual(len(receipt['cleanup_errors']),3)
   self.assertIsNone(receipt['remaining_containers']);self.assertIsNone(receipt['remaining_images'])
if __name__=='__main__':unittest.main()
