import copy,json,pathlib,tempfile,unittest
from unittest.mock import patch
import qualify,verify_linux
from support import digest,load,read,strict

class Evidence(unittest.TestCase):
 def test_frozen_real_capture_and_exact_published_inventory(self):
  here=verify_linux.HERE;verify_linux.verify(here/'evidence-linux')
  provenance=load(here/'linux-provenance.json')
  self.assertEqual(set(provenance),{'schema','files'});self.assertEqual(provenance['schema'],1)
  self.assertEqual(set(provenance['files']),{'build.log','run.log','receipt.json','source-hashes.json'})
  for name,value in provenance['files'].items():self.assertEqual(digest(here/'evidence-linux'/name),value)
 def test_present_exported_artifact_must_match_recorded_identity(self):
  here=verify_linux.HERE
  with tempfile.TemporaryDirectory() as temporary:
   target=pathlib.Path(temporary)
   for p in (here/'evidence-linux').iterdir():
    if p.is_file():(target/p.name).write_bytes(p.read_bytes())
   # Refresh only the verifier binding in this test fixture; all other receipt facts remain intact.
   receipt=load(target/'receipt.json');receipt['source_sha256']['verify_linux.py']=digest(here/'verify_linux.py')
   (target/'receipt.json').write_text(json.dumps(receipt))
   verify_linux.verify(target)
   (target/'rubixctl').write_bytes(b'modified exported executable')
   with self.assertRaisesRegex(ValueError,'exported artifact identity'):verify_linux.verify(target)
 def test_command_order_partial_effects_and_signalled_child_claims_are_required(self):
  raw=read(verify_linux.HERE/'evidence-linux/run.log');line=next(v for v in raw.splitlines() if v.startswith(b'RUBIX_PREPARATION '));record=strict(line[len(b'RUBIX_PREPARATION '):]);mutations=[]
  for index,key,value in [(0,'actions',['/sbin/apk add --no-cache nftables iptables']),(1,'exit',False),(4,'actions',['/sbin/apk add --no-cache nftables iptables','/sbin/rc-update add cgroups boot','/sbin/rc-service cgroups start']),(4,'stderr','error'),(6,'owned_child_absent',False),(6,'signal_sent',None),(3,'exit',0)]:
   changed=copy.deepcopy(record);changed['cases'][index][key]=value;mutations.append(changed)
  changed=copy.deepcopy(record);changed['cases'].pop();mutations.append(changed)
  for value in mutations:
   with self.assertRaises(ValueError):verify_linux.verify_run(raw.replace(line,b'RUBIX_PREPARATION '+json.dumps(value).encode()))
 def test_completion_and_exact_binary_inventory(self):
  raw=read(verify_linux.HERE/'evidence-linux/run.log')
  for changed in [raw.replace(b'6 passed;',b'5 passed;'),raw.replace(b'/out/fixture-command',b'/out/rubixctl'),raw.replace(b'/out/preparation-tests',b'/out/unknown')]:
   with self.assertRaises(ValueError):verify_linux.verify_run(changed)
 def test_missing_harness_fails_before_output_creation(self):
  with tempfile.TemporaryDirectory() as temporary:
   output=pathlib.Path(temporary)/'output'
   with patch.object(qualify,'digest',side_effect=OSError('missing')),patch('sys.argv',['qualify','--output',str(output)]):
    with self.assertRaises(OSError):qualify.main()
   self.assertFalse(output.exists())
 def test_receipt_errors_and_source_changes_cannot_pass(self):
  here=verify_linux.HERE
  with tempfile.TemporaryDirectory() as temporary:
   target=pathlib.Path(temporary)
   for p in (here/'evidence-linux').iterdir():
    if p.is_file():(target/p.name).write_bytes(p.read_bytes())
   original=load(target/'receipt.json')
   for key,value in [('cleanup_errors',['failed']),('remaining_containers',['owned']),('image_id','sha256:bad'),('source_sha256',{})]:
    changed=copy.deepcopy(original);changed[key]=value;(target/'receipt.json').write_text(json.dumps(changed))
    with self.assertRaises(ValueError):verify_linux.verify(target)
if __name__=='__main__':unittest.main()
