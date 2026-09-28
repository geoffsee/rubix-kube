import copy,json,pathlib,shutil,subprocess,sys,tempfile,unittest
from unittest.mock import patch
import capture,verify
class Evidence(unittest.TestCase):
 def test_exact_frozen_go_inventory_and_independent_expected_outcomes(self):
  verify.verify(verify.HERE/'evidence')
  p=verify.load(verify.HERE/'provenance.json')
  self.assertEqual(set(p),{'schema','files'});self.assertEqual(p['schema'],1)
  names={'expected.json','cases.json','source-pins.json'}|{'evidence/'+n for n in ['build.log','run0.log','run1.log','receipt.json','source.sha256']}
  self.assertEqual(set(p['files']),names)
  for name,value in p['files'].items():self.assertEqual(verify.digest(verify.HERE/name),value)
  expected=verify.load(verify.HERE/'expected.json');self.assertEqual(len(expected),56)
  byname={r['name']:r for r in expected}
  self.assertEqual(byname['root_help_check']['outcome'],'help_root')
  self.assertEqual(byname['attached_help_before']['outcome'],'help_check')
  self.assertEqual(byname['help_then_bad']['outcome'],'unknown_flag')
  self.assertFalse(byname["env_'TRUE'"]['install']);self.assertTrue(byname["env_'yes'"]['install'])
 def test_strict_json_and_result_types(self):
  for raw in ['NaN','1e400','-1e400','{"x":0,"x":1}']:
   with self.assertRaises(ValueError):verify.strict(raw)
  with self.assertRaises(ValueError):verify.classify(dict(name='check',stdout='CHECK false false\n',stderr='',exit=False))
  with self.assertRaises(ValueError):verify.equal(True,1,'typed')
 def test_rehashed_semantic_change_fails_independent_oracle(self):
  with tempfile.TemporaryDirectory() as tmp:
   dst=pathlib.Path(tmp)/'evidence';shutil.copytree(verify.HERE/'evidence',dst)
   r=verify.load(dst/'receipt.json')
   for name in ['run0.log','run1.log']:
    p=dst/name;p.write_text(p.read_text().replace('CHECK false false','CHECK true false'));r['outputs'][name]=verify.digest(p)
   (dst/'receipt.json').write_text(json.dumps(r))
   with self.assertRaisesRegex(ValueError,'independent parser expectations'):verify.verify(dst)
 def test_missing_inventory_or_cleanup_is_rejected_under_optimized_python(self):
  with tempfile.TemporaryDirectory() as tmp:
   dst=pathlib.Path(tmp)/'evidence';shutil.copytree(verify.HERE/'evidence',dst)
   original=verify.load(dst/'receipt.json');mutations=[]
   for field in ['outputs','source_sha256']:
    value=copy.deepcopy(original);value[field].pop(next(iter(value[field])));mutations.append(value)
   for field,value in [('remaining_containers',None),('cleanup_errors',['failed']),('identical_records',1),('extra_claim',True)]:
    r=copy.deepcopy(original);r[field]=value;mutations.append(r)
   for r in mutations:
    (dst/'receipt.json').write_text(json.dumps(r))
    result=subprocess.run([sys.executable,'-O',str(verify.HERE/'verify.py'),str(dst)],capture_output=True,timeout=10)
    self.assertNotEqual(result.returncode,0)
 def test_failed_setup_retains_independent_cleanup_diagnostics(self):
  with tempfile.TemporaryDirectory() as tmp:
   out=pathlib.Path(tmp)/'capture'
   with patch.object(sys,'argv',['capture.py','--output',str(out)]),patch.object(capture.helper,'bounded',side_effect=RuntimeError('setup failed')),patch.object(capture.helper.subprocess,'run',side_effect=RuntimeError('daemon absent')) as cleanup:
    self.assertEqual(capture.main(),1)
   r=verify.load(out/'receipt.json');self.assertEqual(cleanup.call_count,3);self.assertEqual(len(r['cleanup_errors']),3);self.assertIsNone(r['remaining_containers']);self.assertIsNone(r['remaining_images'])
if __name__=='__main__':unittest.main()
