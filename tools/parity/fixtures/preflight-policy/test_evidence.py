import hashlib,json,pathlib,tempfile,unittest
from unittest.mock import patch
import capture,verify
class Evidence(unittest.TestCase):
 def test_current_frozen_evidence(self):verify.verify()
 def test_duplicate_nonfinite(self):
  for data in ['{"x":1,"x":2}','NaN','1e400']:
   with self.assertRaises(ValueError):verify.loads(data)
 def test_rehashed_observation_mutation_is_rejected(self):
  original=verify.read
  def changed(path):
   data=original(path)
   if path.name=='run0.log':return data.replace(b'hostname_uppercase\\tfalse',b'hostname_uppercase\\ttrue')
   return data
  def rehashed(path):return hashlib.sha256(changed(path)).hexdigest()
  with patch.object(verify,'read',side_effect=changed),patch.object(verify,'digest',side_effect=lambda path:hashlib.sha256(original(path)).hexdigest()):
   with self.assertRaisesRegex(ValueError,'independent reference'):verify.verify()
 def test_cleanup_failure_still_publishes_receipt(self):
  with tempfile.TemporaryDirectory() as directory:
   report={'containers':['owned0','owned1'],'cleanup_errors':[]}
   with patch.object(capture.helper.subprocess,'run',side_effect=OSError('daemon unavailable')) as run:capture.helper.finish(report,pathlib.Path(directory),'owned-image')
   self.assertEqual(run.call_count,5)
   receipt=verify.loads((pathlib.Path(directory)/'receipt.json').read_bytes())
   self.assertEqual(len(receipt['cleanup_errors']),5);self.assertIsNone(receipt['remaining_containers']);self.assertIsNone(receipt['remaining_images'])
 def test_expected_or_source_edit_requires_recapture(self):
  original=verify.digest
  for name in ['expected.tsv','go.mod','preflight_capture_test.go','Capture.Dockerfile']:
   with self.subTest(name=name),patch.object(verify,'digest',side_effect=lambda path:'0'*64 if path==verify.HERE/name else original(path)),self.assertRaises(ValueError):verify.verify()
 def test_mutated_receipt_and_missing_entries_fail(self):
  original=verify.loads
  for key in ['errors','identical_records','containers']:
   def mutation(data):
    value=original(data)
    if type(value) is dict and 'archive_sha256' in value:
     if key=='errors':value[key]=['failure']
     elif key=='identical_records':value[key]=1
     else:value[key]=value[key][:1]
    return value
   with self.subTest(key=key),patch.object(verify,'loads',side_effect=mutation),self.assertRaises(ValueError):verify.verify()
if __name__=='__main__':unittest.main()
