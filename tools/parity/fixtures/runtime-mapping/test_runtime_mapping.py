import copy
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import capture
import verify

HERE=pathlib.Path(__file__).resolve().parent
SOURCES={'mapping_capture_test.go','Capture.Dockerfile','capture.py','verify.py'}
BASELINE={'go.mod','go.sum','internal/config/embedded.go','internal/config/defaults.go','internal/runtime/cri/endpoint.go','types/types.go','types/config.go','types/const.go','types/var.go'}
EVIDENCE={'build.log','receipt.json','mapping.json','mapping-0.log','mapping-1.log'}

def inventories(receipt,provenance):
 verify.equal(set(receipt['source_sha256']),SOURCES)
 verify.equal(set(receipt['outputs']),{'mapping.json'})
 verify.equal(set(provenance['source_sha256']),BASELINE)
 verify.equal(set(provenance['fixture_sha256']),{'test_runtime_mapping.py','README.md'}|{'evidence/'+name for name in EVIDENCE})

def repeats(directory):
 value=verify.load(directory/'mapping.json');verify.verify(value)
 for index in range(2):
  path=directory/f'mapping-{index}.log'
  with path.open('rb') as source:data=source.read(verify.LIMIT+1)
  if len(data)>verify.LIMIT:raise ValueError('log exceeds limit')
  records=[line.removeprefix('RUBIX_CAPTURE ') for line in data.decode().splitlines() if line.startswith('RUBIX_CAPTURE ')]
  verify.equal(len(records),1);verify.equal(verify.loads(records[0]),value)

class RuntimeMapping(unittest.TestCase):
 def test_actual_mapping_matches_independent_complete_expectations(self):
  verify.verify(verify.load(HERE/'evidence/mapping.json'))
 def test_path_probe_hostname_endpoint_and_type_mutations_fail(self):
  changes=[(0,['embedded','KineSocketFile'],'/var/lib/kubesolo/kine/kine.sock'),(0,['embedded','KubeletCerts','CACert'],'/wrong/ca'),(1,['embedded','NodeName'],'  Talos-CP-1  '),(1,['embedded','NodeIP'],'198.51.100.1'),(1,['embedded','MTU'],9000),(1,['embedded','ContainerMode'],False),(1,['embedded','IsPortainerEdge'],0),(4,['embedded','NodeName'],'mixed-host'),(5,['error'],'empty hostname rejected'),(6,['embedded','RuntimeExternal'],False),(7,['embedded','RuntimeSocketPath'],'/run/runtime.sock'),(9,['error'],''),(12,['embedded','RuntimeEndpoint'],'unix://')]
  for index,path,new in changes:
   value=verify.expected();target=value[index]
   for key in path[:-1]:target=target[key]
   target[path[-1]]=new
   with self.subTest(index=index,path=path),self.assertRaises(ValueError):verify.verify(value)
 def test_missing_extra_and_reordered_cases_fail(self):
  value=verify.expected()
  for changed in (value[:-1],value+[value[0]],list(reversed(value))):
   with self.assertRaises(ValueError):verify.verify(changed)
  del value[0]['embedded']['RuntimeCgroupDriver']
  with self.assertRaises(ValueError):verify.verify(value)
 def test_optimized_cli_rejects_changed_behavior(self):
  with tempfile.TemporaryDirectory() as temporary:
   path=pathlib.Path(temporary)/'mapping.json';path.write_text(__import__('json').dumps(verify.expected()))
   command=[sys.executable,'-O',str(HERE/'verify.py'),str(path)]
   self.assertEqual(subprocess.run(command,capture_output=True).returncode,0)
   value=verify.expected();value[0]['embedded']['RuntimeExternal']=1;path.write_text(__import__('json').dumps(value))
   self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
 def test_json_bounds_duplicates_and_nonfinite_rejected(self):
  for text in ('{"x":1,"x":2}','{"x":NaN}','{"x":Infinity}','{"x":1e400}','{"x":-1e400}',' '* (verify.LIMIT+1)):
   with self.assertRaises(ValueError):verify.loads(text)
  with tempfile.TemporaryDirectory() as temporary:
   path=pathlib.Path(temporary)/'large';path.write_bytes(b' '*(verify.LIMIT+1))
   with self.assertRaises(ValueError):verify.load(path)
 def test_current_exact_provenance_receipt_and_repeats(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json');inventories(receipt,provenance);repeats(HERE/'evidence')
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):verify.equal(receipt[key],[])
  verify.equal(receipt['identical_repeats'],True)
  verify.equal(receipt['revision'],provenance['revision']);verify.equal(receipt['helper_sha256'],capture.digest(capture.ROOT/'tools/defaults/capture.py'))
  for name,digest in receipt['source_sha256'].items():verify.equal(capture.digest(HERE/name),digest)
  for name,digest in receipt['outputs'].items():verify.equal(capture.digest(HERE/'evidence'/name),digest)
  for name,digest in provenance['fixture_sha256'].items():verify.equal(capture.digest(HERE/name),digest)
 def test_removed_inventories_or_missing_duplicate_changed_repeats_fail(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json')
  for owner,key in [('receipt','source_sha256'),('receipt','outputs'),('provenance','source_sha256'),('provenance','fixture_sha256')]:
   r=copy.deepcopy(receipt);p=copy.deepcopy(provenance);mapping=(r if owner=='receipt' else p)[key];del mapping[next(iter(mapping))]
   with self.assertRaises(ValueError):inventories(r,p)
  for kind in ('missing','duplicate','changed'):
   with tempfile.TemporaryDirectory() as temporary:
    root=pathlib.Path(temporary)
    for name in ('mapping.json','mapping-0.log','mapping-1.log'):(root/name).write_bytes((HERE/'evidence'/name).read_bytes())
    path=root/'mapping-1.log';lines=path.read_text().splitlines();record=next(line for line in lines if line.startswith('RUBIX_CAPTURE '))
    if kind=='missing':lines.remove(record)
    elif kind=='duplicate':lines.append(record)
    else:
     value=verify.loads(record.removeprefix('RUBIX_CAPTURE '));value[0]['embedded']['NodeIPSpecified']=0;lines[lines.index(record)]='RUBIX_CAPTURE '+__import__('json').dumps(value)
    path.write_text('\n'.join(lines)+'\n')
    with self.assertRaises(ValueError):repeats(root)
 def test_teardown_failure_still_publishes_receipt(self):
  with tempfile.TemporaryDirectory() as temporary:
   report={'containers':['owned'],'cleanup_errors':[]}
   with mock.patch.object(capture.helper.subprocess,'run',side_effect=OSError('unavailable')):capture.helper.finish(report,pathlib.Path(temporary),'owned')
   value=verify.load(pathlib.Path(temporary)/'receipt.json');self.assertEqual(len(value['cleanup_errors']),4);self.assertIsNone(value['remaining_containers']);self.assertIsNone(value['remaining_images'])
if __name__=='__main__':unittest.main()
