import json,pathlib,shutil,tempfile,unittest
from unittest.mock import patch
import capture,verify

class EvidenceTests(unittest.TestCase):
 def test_frozen_actual_captures(self):verify.verify()
 def test_nonfinite_and_duplicate_json(self):
  for text in ['NaN','1e400','-1e400','{"x":1,"x":2}']:
   with self.subTest(text=text),self.assertRaises(ValueError):verify.loads(text)
 def test_missing_cleanup_inventory_is_not_success(self):
  report={key:[] for key in ['errors','cleanup_errors','remaining_containers','remaining_images']};report['containers']=['owned']
  for key in report:
   changed=dict(report);del changed[key]
   with self.subTest(key=key),self.assertRaises(ValueError):verify.clean(changed)
 def test_linux_completion_cannot_be_removed_even_with_fresh_hashes(self):
  with tempfile.TemporaryDirectory() as temporary:
   here=pathlib.Path(temporary)/'copy';shutil.copytree(verify.HERE,here)
   log=here/'evidence/linux/run.log';log.write_bytes(log.read_bytes().replace(b'test result: ok. 8 passed;',b'test result: FAILED. 0 passed;'))
   receipt=here/'evidence/linux/receipt.json';report=verify.loads(receipt.read_bytes());report['run_sha256']=verify.digest(log);receipt.write_text(json.dumps(report))
   provenance=verify.loads((here/'provenance.json').read_bytes());provenance['files']['linux/run.log']=verify.digest(log);provenance['files']['linux/receipt.json']=verify.digest(receipt);(here/'provenance.json').write_text(json.dumps(provenance))
   with self.assertRaisesRegex(ValueError,'Linux completion'):verify.verify(here)
 def test_rehashed_duplicate_actual_record_is_rejected(self):
  with tempfile.TemporaryDirectory() as temporary:
   here=pathlib.Path(temporary)/'copy';shutil.copytree(verify.HERE,here)
   log=here/'evidence/go/run1.log';data=log.read_bytes();line=next(line for line in data.splitlines() if line.startswith(b'RUBIX_CAPTURE '));log.write_bytes(data+line+b'\n')
   receipt=here/'evidence/go/receipt.json';report=verify.loads(receipt.read_bytes());report['outputs']['run1.log']=verify.digest(log);receipt.write_text(json.dumps(report))
   provenance=verify.loads((here/'provenance.json').read_bytes());provenance['files']['go/run1.log']=verify.digest(log);provenance['files']['go/receipt.json']=verify.digest(receipt);(here/'provenance.json').write_text(json.dumps(provenance))
   with self.assertRaisesRegex(ValueError,'exact independent Go records'):verify.verify(here)
 def test_current_dependency_and_probe_changes_require_recapture(self):
  original=verify.digest
  for name in ['Cargo.lock','crates/rubix-platform/src/discover.rs','tools/parity/fixtures/preflight-policy/expected.tsv']:
   with self.subTest(name=name),patch.object(verify,'digest',side_effect=lambda path:'0'*64 if path==verify.ROOT/name else original(path)),self.assertRaises(ValueError):verify.verify()
 def test_cleanup_continues_and_reports_unavailable_inventories(self):
  with tempfile.TemporaryDirectory() as temporary:
   report={'containers':['owned'],'cleanup_errors':[]}
   with patch.object(capture.helper.subprocess,'run',side_effect=OSError('daemon unavailable')) as command:capture.helper.finish(report,pathlib.Path(temporary),'owned-image')
   self.assertEqual(command.call_count,4);self.assertEqual(len(report['cleanup_errors']),4);self.assertIsNone(report['remaining_images']);self.assertIsNone(report['remaining_containers']);self.assertTrue((pathlib.Path(temporary)/'receipt.json').is_file())
if __name__=='__main__':unittest.main()
