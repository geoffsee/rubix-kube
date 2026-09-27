import copy,hashlib,json,os,pathlib,shutil,subprocess,sys,tempfile,unittest
from unittest import mock
import verify,capture
HERE=pathlib.Path(__file__).resolve().parent
class Startup(unittest.TestCase):
 def test_capture_rejects_changed_selected_runner_or_driver_before_execution(self):
  runner=HERE.parents[3]/'tools/parity/run.py'
  self.assertEqual(capture.check_runner(runner),runner.resolve())
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)
   for name in ('run.py','driver.py'):shutil.copyfile(runner.parent/name,root/name)
   for name in ('run.py','driver.py'):
    original=(root/name).read_bytes();(root/name).write_bytes(original+b'\n# altered\n')
    with self.subTest(name=name),self.assertRaises(ValueError):capture.check_runner(root/'run.py')
    (root/name).write_bytes(original)
 def test_current_baseline_captures_match(self):
  for name in ('r3','r4'):verify.verify(HERE/'evidence'/name)
 def test_hashbound_current_suite_and_harness(self):
  provenance=verify.load(HERE/'provenance.json')
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(verify.digest(HERE/name),digest)
  runner=pathlib.Path(os.environ.get('RUBIX_PARITY_RUNNER',str(HERE.parents[3]/'tools/parity/run.py')))
  for name,digest in provenance['runner_source_sha256'].items():self.assertEqual(verify.digest(runner.parent/name),digest)
 def test_only_log_timestamp_is_normalized(self):
  self.assertEqual(verify.normalize('{"time":"one","message":"value"}\n'),verify.normalize('{"time":"two","message":"value"}\n'))
  self.assertNotEqual(verify.normalize('{"time":"one","version":"1"}\n'),verify.normalize('{"time":"one","version":"2"}\n'))
  self.assertNotEqual(verify.normalize('plain\n'),verify.normalize('plain'))
 def test_type_and_inventory_changes_fail(self):
  for a,b in [(0,False),({}, {'a':None}),([],{}),(1,1.0)]:
   with self.assertRaises(ValueError):verify.equal(a,b)
 def test_malformed_json_rejected(self):
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d)/'bad'
   for s in ['{"a":1,"a":2}','{"a":NaN}']:
    p.write_text(s)
    with self.assertRaises(ValueError):verify.load(p)
 def test_raw_semantic_mutation_fails_even_with_updated_stream_hash(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)/'capture';shutil.copytree(HERE/'evidence/r3',root)
   result=verify.load(root/'result.json');case=next(c for c in result['cases'] if c['id']=='version-invalid-bool-env')
   (root/case['stderr_file']).write_text('incorrectly rejected invalid environment\n')
   case['stderr_sha256']=verify.digest(root/case['stderr_file']);(root/'result.json').write_text(json.dumps(result))
   with self.assertRaises(ValueError):verify.verify(root)
 def test_repeat_cleanup_and_missing_case_negatives(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)/'capture';shutil.copytree(HERE/'evidence/r3',root)
   initial=verify.load(root/'result.json')
   for kind in ('bool-exit','missing','cleanup'):
    value=copy.deepcopy(initial)
    if kind=='bool-exit':value['cases'][0]['exit_code']=False
    elif kind=='missing':value['cases'].pop()
    else:value['cases'][0]['owned_process_group_absent']=False
    (root/'result.json').write_text(json.dumps(value))
    with self.assertRaises(ValueError):verify.verify(root)
 def test_changed_runner_identity_fails(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)/'capture';shutil.copytree(HERE/'evidence/r3',root)
   value=verify.load(root/'runner-result.json');value['source_sha256']['driver.py']='0'*64;(root/'runner-result.json').write_text(json.dumps(value))
   with self.assertRaises(ValueError):verify.verify(root)
 def test_provenance_inventories_cannot_drop_hashes(self):
  initial=verify.load(HERE/'provenance.json')
  for family in ('fixture_sha256','runner_source_sha256','source_sha256','kingpin_source_sha256'):
   value=copy.deepcopy(initial);value[family].pop(next(iter(value[family])))
   with self.subTest(family=family),self.assertRaises(ValueError):verify.validate_provenance(value)
  value=copy.deepcopy(initial);value['fixture_sha256']['unexpected']='0'*64
  with self.assertRaises(ValueError):verify.validate_provenance(value)
 def test_duplicate_cases_and_missing_receipt_hash_fail(self):
  with tempfile.TemporaryDirectory() as d:
   root=pathlib.Path(d)/'capture';shutil.copytree(HERE/'evidence/r3',root)
   value=verify.load(root/'result.json');value['cases'][1]=copy.deepcopy(value['cases'][0]);(root/'result.json').write_text(json.dumps(value))
   with self.assertRaises(ValueError):verify.inspect(root)
   shutil.copyfile(HERE/'evidence/r3/result.json',root/'result.json')
   value=verify.load(root/'runner-result.json');del value['source_sha256']['artifact_descriptor'];(root/'runner-result.json').write_text(json.dumps(value))
   with self.assertRaises(ValueError):verify.inspect(root)
 def test_normalization_rejects_duplicate_and_nonfinite_json_logs(self):
  for text in ('{"time":1,"time":2,"message":"x"}\n','{"message":"x","message":"y"}\n','{"time":NaN}\n','{"time":1e999}\n','{"nested":{"value":Infinity}}\n'):
   with self.subTest(text=text),self.assertRaises(ValueError):verify.normalize(text)
  self.assertEqual(verify.normalize('plain {diagnostic}\n'),['plain {diagnostic}\n'])
  self.assertEqual(verify.normalize('{not json}\n'),['{not json}\n'])
 def test_stream_and_json_reads_are_bounded(self):
  with mock.patch.object(verify,'JSON_LIMIT',8):
   with self.assertRaises(ValueError):verify.load(HERE/'evidence/r3/result.json')
  with mock.patch.object(verify,'STREAM_LIMIT',8):
   with self.assertRaises(ValueError):verify.inspect(HERE/'evidence/r3')
 def test_optimized_cli_requires_actual_evidence(self):
  command=[sys.executable,'-O',str(HERE/'verify.py')]
  self.assertEqual(subprocess.run(command+[str(HERE/'evidence/r3')],capture_output=True).returncode,0)
  with tempfile.TemporaryDirectory() as d:self.assertNotEqual(subprocess.run(command+[d],capture_output=True).returncode,0)
if __name__=='__main__':unittest.main()
