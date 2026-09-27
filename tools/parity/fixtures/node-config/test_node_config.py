import copy,hashlib,json,pathlib,shutil,subprocess,sys,tempfile,unittest
from unittest import mock
import capture,verify
HERE=pathlib.Path(__file__).resolve().parent
COMPONENTS=('kubelet','containerd')
SOURCE_FILES={'kubelet_capture_test.go','containerd_capture_test.go','Capture.Dockerfile','capture.py'}
BASELINE_FILES={'go.mod','go.sum','pkg/kubernetes/kubelet/config.go','pkg/kubernetes/kubelet/args.go','pkg/kubernetes/kubelet/service.go','pkg/runtime/containerd/config.go','pkg/runtime/containerd/service.go','internal/runtime/network/ip.go','internal/runtime/filesystem/file.go','types/const.go'}
EVIDENCE_FILES={'build.log','receipt.json'}|{name+suffix for name in COMPONENTS for suffix in ('.json','-0.log','-1.log')}
def inventories(receipt,provenance):
 verify.equal(set(receipt['source_sha256']),SOURCE_FILES);verify.equal(set(receipt['outputs']),{name+'.json' for name in COMPONENTS});verify.equal(set(provenance['source_sha256']),BASELINE_FILES);verify.equal(set(provenance['expected_sha256']),set(COMPONENTS))
 verify.equal(set(provenance['fixture_sha256']),{'verify.py','test_node_config.py'}|{'evidence/'+name for name in EVIDENCE_FILES}|{'expected/'+name+'.json' for name in COMPONENTS})
def repeats(directory):
 for component in COMPONENTS:
  expected=verify.load(directory/(component+'.json'));verify.verify(expected,component)
  for index in range(2):
   records=[line.removeprefix('RUBIX_CAPTURE ') for line in (directory/f'{component}-{index}.log').read_text().splitlines() if line.startswith('RUBIX_CAPTURE ')]
   verify.equal(len(records),1)
   with tempfile.TemporaryDirectory() as temporary:
    path=pathlib.Path(temporary)/'record.json';path.write_text(records[0]);value=verify.load(path)
   verify.equal(value,expected)
class NodeConfig(unittest.TestCase):
 def test_captured_semantics(self):
  for component in COMPONENTS:verify.verify(verify.load(HERE/'evidence'/(component+'.json')),component)
 def test_security_runtime_checkpoint_and_argument_mutations_fail(self):
  changes=[('kubelet',['variants','host_default','config','authentication','anonymous','enabled'],True),('kubelet',['variants','reported_systemd','config','cgroupDriver'],'cgroupfs'),('kubelet',['variants','container_default','rendered_config','cgroupsPerQOS'],0),('kubelet',['checkpoint_states','same'],'changed'),('kubelet',['checkpoint_states','options'],'unchanged'),('kubelet',['checkpoint_negative_control'],'unchanged'),('kubelet',['args','127.0.0.1'],['--node-ip','127.0.0.1']),('containerd',['config','plugins','io.containerd.cri.v1.runtime','containerd','runtimes','crun','runtime_type'],'/fixture/shim'),('containerd',['checks','missing_parent_fails'],False)]
  for component,path,new in changes:
   value=verify.load(HERE/'evidence'/(component+'.json'));target=value
   for key in path[:-1]:target=target[key]
   target[path[-1]]=new
   with self.subTest(path=path),self.assertRaises(ValueError):verify.verify(value,component)
 def test_runtime_path_and_missing_case_fail(self):
  value=verify.load(HERE/'evidence/containerd.json');value['config']['plugins']['io.containerd.cri.v1.runtime']['containerd']['runtimes']['crun']['runtime_path']='/fixture/shim'
  with self.assertRaises(ValueError):verify.verify(value,'containerd')
  value=verify.load(HERE/'evidence/kubelet.json');del value['variants']['host_default']
  with self.assertRaises(ValueError):verify.verify(value,'kubelet')
 def test_optimized_cli_rejects_changed_render_and_swapped_family(self):
  with tempfile.TemporaryDirectory() as temporary:
   root=pathlib.Path(temporary)
   for component in COMPONENTS:shutil.copyfile(HERE/'evidence'/(component+'.json'),root/(component+'.json'))
   command=[sys.executable,'-O',str(HERE/'verify.py'),str(root)]
   self.assertEqual(subprocess.run(command,capture_output=True).returncode,0)
   value=verify.load(root/'containerd.json');value['toml']+='\n# altered\n';(root/'containerd.json').write_text(json.dumps(value));self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
   shutil.copyfile(root/'kubelet.json',root/'containerd.json');self.assertNotEqual(subprocess.run(command,capture_output=True).returncode,0)
 def test_strict_json(self):
  with tempfile.TemporaryDirectory() as temporary:
   path=pathlib.Path(temporary)/'bad'
   for text in ('{"a":1,"a":2}','{"a":NaN}'):
    path.write_text(text)
    with self.assertRaises(ValueError):verify.load(path)
 def test_current_receipt_and_provenance(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json');inventories(receipt,provenance);repeats(HERE/'evidence')
  for key in ('errors','cleanup_errors','remaining_containers','remaining_images'):self.assertEqual(receipt[key],[])
  self.assertIs(receipt['identical_repeats'],True);self.assertEqual(receipt['helper_sha256'],capture.digest(capture.ROOT/'tools/defaults/capture.py'))
  for name,digest in receipt['source_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
  for name,digest in receipt['outputs'].items():self.assertEqual(capture.digest(HERE/'evidence'/name),digest)
  for name,digest in provenance['fixture_sha256'].items():self.assertEqual(capture.digest(HERE/name),digest)
 def test_removed_inventory_and_changed_repeat_fail(self):
  receipt=verify.load(HERE/'evidence/receipt.json');provenance=verify.load(HERE/'provenance.json')
  for owner,key in [('receipt','source_sha256'),('receipt','outputs'),('provenance','source_sha256'),('provenance','expected_sha256'),('provenance','fixture_sha256')]:
   r=copy.deepcopy(receipt);p=copy.deepcopy(provenance);mapping=(r if owner=='receipt' else p)[key];del mapping[next(iter(mapping))]
   with self.assertRaises(ValueError):inventories(r,p)
  for kind in ('missing','duplicate','changed'):
   with tempfile.TemporaryDirectory() as temporary:
    root=pathlib.Path(temporary)
    for name in EVIDENCE_FILES:shutil.copyfile(HERE/'evidence'/name,root/name)
    path=root/'kubelet-1.log';lines=path.read_text().splitlines();record=next(line for line in lines if line.startswith('RUBIX_CAPTURE '))
    if kind=='missing':lines.remove(record)
    elif kind=='duplicate':lines.append(record)
    else:
     value=json.loads(record.removeprefix('RUBIX_CAPTURE '));value['failures']['parent_file']=1;lines[lines.index(record)]='RUBIX_CAPTURE '+json.dumps(value)
    path.write_text('\n'.join(lines)+'\n')
    with self.assertRaises(ValueError):repeats(root)
 def test_cleanup_failure_reports(self):
  with tempfile.TemporaryDirectory() as temporary:
   report={'containers':['owned'],'cleanup_errors':[]}
   with mock.patch.object(capture.helper.subprocess,'run',side_effect=OSError('unavailable')):capture.helper.finish(report,pathlib.Path(temporary),'owned')
   result=verify.load(pathlib.Path(temporary)/'receipt.json');self.assertEqual(len(result['cleanup_errors']),4);self.assertIsNone(result['remaining_containers'])
if __name__=='__main__':unittest.main()
