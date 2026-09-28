import copy, hashlib, json, pathlib, shutil, subprocess, sys, tempfile, unittest
from unittest.mock import patch
import capture, verify

class Evidence(unittest.TestCase):
 def test_frozen_capture_and_exact_inventory(self):
  verify.verify(verify.HERE/'evidence')
  provenance=verify.load(verify.HERE/'provenance.json')
  self.assertEqual(set(provenance),{'files','schema'})
  self.assertEqual(provenance['schema'],1)
  expected={'expected.json','source-pins.json'}|{'evidence/'+n for n in ['receipt.json','source.sha256','build.log']+[f'{f}{i}.log' for f in verify.FAMILIES for i in range(2)]}
  self.assertEqual(set(provenance['files']),expected)
  for name,digest in provenance['files'].items(): self.assertEqual(verify.digest(verify.HERE/name),digest)
 def test_independent_expectations_keep_baseline_weaknesses_visible(self):
  expected=verify.load(verify.HERE/'expected.json')
  self.assertIn('unreadable_skipped\ttrue',expected['network'])
  self.assertIn('prefix_one_accepted\ttrue',expected['network'])
  self.assertIn('incorrect_readonly\tfalse',expected['network'])
  self.assertIn('unreadable\tnftables',expected['proxy'])
  self.assertIn('failed_assumes_nft\ttrue',expected['system'])
  self.assertIn('owned\t10-bridge.conflist',expected['embedded'])
 def test_strict_types_nonfinite_duplicates_and_record_count(self):
  for raw in ['{"a":1,"a":2}','NaN','1e400','-1e400']:
   with self.assertRaises(ValueError):verify.strict(raw)
  for a,b in [(True,1),(False,0),(['1'],[1])]:
   with self.assertRaises(ValueError):verify.equal(a,b,'type')
  with tempfile.TemporaryDirectory() as tmp:
   path=pathlib.Path(tmp)/'run'
   for raw in ['no record','RUBIX_CAPTURE []\nRUBIX_CAPTURE []\n']:
    path.write_text(raw)
    with self.assertRaises(ValueError):verify.records(path)
 def test_mutated_records_and_missing_inventory_fail_under_optimized_python(self):
  with tempfile.TemporaryDirectory() as tmp:
   directory=pathlib.Path(tmp)/'evidence';shutil.copytree(verify.HERE/'evidence',directory)
   original=verify.load(directory/'receipt.json')
   changes=[]
   for field,value in [('identical_records',1),('errors',['failed']),('remaining_images',None)]:
    changed=copy.deepcopy(original);changed[field]=value;changes.append(changed)
   for field in ['outputs','source_sha256']:
    changed=copy.deepcopy(original);changed[field].pop(next(iter(changed[field])));changes.append(changed)
   for receipt in changes:
    (directory/'receipt.json').write_text(json.dumps(receipt))
    result=subprocess.run([sys.executable,'-O',str(verify.HERE/'verify.py'),str(directory)],capture_output=True,timeout=10)
    self.assertNotEqual(result.returncode,0)
   # Rehash a dishonest actual observation: independent expected semantics still reject.
   path=directory/'network0.log';path.write_text(path.read_text().replace('incorrect_readonly\\tfalse','incorrect_readonly\\ttrue'))
   original['outputs'][path.name]=verify.digest(path);(directory/'receipt.json').write_text(json.dumps(original))
   with self.assertRaisesRegex(ValueError,'network observations'):verify.verify(directory)
 def test_setup_failure_still_attempts_cleanup_and_publishes_unknown(self):
  with tempfile.TemporaryDirectory() as tmp:
   output=pathlib.Path(tmp)/'output'
   with patch.object(sys,'argv',['capture.py','--output',str(output)]),patch.object(capture.helper,'bounded',side_effect=RuntimeError('setup failed')),patch.object(capture.helper.subprocess,'run',side_effect=RuntimeError('daemon unavailable')) as cleanup:
    self.assertEqual(capture.main(),1)
   receipt=verify.load(output/'receipt.json');self.assertEqual(cleanup.call_count,3)
   self.assertIsNone(receipt['remaining_containers']);self.assertIsNone(receipt['remaining_images'])
   self.assertEqual(len(receipt['cleanup_errors']),3)
 def test_source_pins_match_five_original_readonly_files(self):
  pins=verify.load(verify.HERE/'source-pins.json')
  self.assertEqual(set(pins),{'internal/runtime/network/ipv6.go','internal/system/modules.go','pkg/kubernetes/kubeproxy/flags.go','internal/core/embedded/host.go','types/const.go'})
  ref=pathlib.Path('/Volumes/safe-vol/workspace/archives/playground/rubix-kube/.agents/skills/kubesolo-rewriter/references/kubesolo')
  if ref.is_dir():
   for name,digest in pins.items():self.assertEqual(hashlib.sha256((ref/name).read_bytes()).hexdigest(),digest)
if __name__=='__main__':unittest.main()
