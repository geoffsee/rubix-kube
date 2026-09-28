import importlib.util
from pathlib import Path
import tempfile
import unittest
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('node_verify',HERE/'verify.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
def sample():
 lines=['a'*64+'  /out/'+name for name in ['host_preflight','iptables_probe','assess_host']]
 lines+=['test result: ok. 9 passed; 0 failed; 1 ignored;','test result: ok. 1 passed; 0 failed; 0 ignored;','test result: ok. 1 passed; 0 failed; 0 ignored;','RUBIX_NODE_PROBE_COMPLETE cases=11 external_sentinel_preserved=true','RUBIX_NODE_ASSESSMENT external=true real_probe=true injected_host_facts=true']
 for name,status in v.EXPECTED.items():lines.append(f'RUBIX_NODE_PROBE case={name} result={status} joined=true reaped={"false" if name in ["missing","denied"] else "true"} elapsed_ms={2100 if name=="deadline" else 100}')
 for name,code in [('help',0),('version',0),('print',0),('blocked',1)]:lines.append(f'RUBIX_NODE_CONSUMER case={name} exit={code} probe_absent=true')
 lines.append('RUBIX_NAMESPACE {"init":1,"shell":2,"helper":3,"processes":[1,2,3]}')
 return '\n'.join(lines)
class Evidence(unittest.TestCase):
 def parse(self,text):
  with tempfile.TemporaryDirectory() as tmp:
   path=Path(tmp)/'run';path.write_text(text);return v.records(path)
 def test_expected_observations(self):self.assertEqual(len(self.parse(sample())[0]),11)
 def test_adverse_probe_claims_are_required(self):
  for old,new in [('Some(Deadline)','None'),('joined=true','joined=false'),('elapsed_ms=2100','elapsed_ms=100'),('external_sentinel_preserved=true','external_sentinel_preserved=false'),('probe_absent=true','probe_absent=false')]:
   with self.subTest(old=old),self.assertRaises(ValueError):self.parse(sample().replace(old,new,1))
 def test_namespace_identity_and_extra_process(self):
  for old,new in [('"shell":2','"shell":1'),('[1,2,3]','[1,2,3,4]')]:
   with self.assertRaises(ValueError):self.parse(sample().replace(old,new))
 def test_binary_and_case_inventory(self):
  for text in [sample().replace('  /out/assess_host','  /out/other'),sample()+'\n'+sample().splitlines()[8]]:
   with self.assertRaises(ValueError):self.parse(text)
 def test_strict_json(self):
  for text in ['{"x":1,"x":2}','{"x":1e400}','{"x":NaN}']:
   with tempfile.TemporaryDirectory() as tmp:
    path=Path(tmp)/'json';path.write_text(text)
    with self.assertRaises(ValueError):v.load(path)
 def test_owned_recipe_cannot_expand_privilege_or_change_identity(self):
  import copy
  tag='rubix-node-assessment-'+'a'*32
  report=dict(tag=tag,containers=[tag+'-first',tag+'-repeat'],runs={name:dict(command=v.run_command(tag,name)) for name in ['first','repeat']})
  v.ownership(report)
  for field in ['containers','privilege','name']:
   changed=copy.deepcopy(report)
   if field=='containers':changed['containers']=[]
   elif field=='privilege':changed['runs']['first']['command'][5]='--privileged'
   else:changed['runs']['repeat']['command']=v.run_command(tag,'first')
   with self.assertRaises(ValueError):v.ownership(changed)
 def test_frozen_capture(self):v.capture(HERE/'evidence')
if __name__=='__main__':unittest.main()
