import importlib.util
from pathlib import Path
import tempfile
import unittest
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('output_verify',HERE/'verify.py')
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
def log():
    lines=['a'*64+'  /output-tests','test result: ok. 1 passed; 0 failed; 1 filtered out;', 'RUBIX_OUTPUT_COMPLETE cases=13']
    for name,(status,size) in v.EXPECTED.items():
        lines.append(f'RUBIX_OUTPUT case={name} status={status} bytes={size or 0} reaped={"false" if name=="missing" else "true"} joined=true elapsed_ms=30')
    lines.append('RUBIX_NAMESPACE {"init":1,"shell":2,"helper":3,"processes":[1,2,3]}')
    return '\n'.join(lines)
class Evidence(unittest.TestCase):
    def parse(self,text):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'run.log';path.write_text(text);return v.records(path)
    def test_valid_independent_expected_records(self): self.assertEqual(len(self.parse(log())[0]),13)
    def test_failures_are_not_complete_or_reaped_claims(self):
        for old,new in [('status=LimitExceeded','status=Complete'),('joined=true','joined=false'),('reaped=true','reaped=false'),('cases=13','cases=12'),('"processes":[1,2,3]','"processes":[1,2,3,4]')]:
            with self.subTest(old=old),self.assertRaises(ValueError): self.parse(log().replace(old,new,1))
    def test_limits_and_duplicate_records(self):
        for text in [log().replace('bytes=64','bytes=65',1),log().replace('elapsed_ms=30','elapsed_ms=5000',1),log()+'\n'+log().splitlines()[3]]:
            with self.assertRaises(ValueError): self.parse(text)
    def test_namespace_roles_are_distinct_positive_processes(self):
        for old,new in [('"shell":2,"helper":3,"processes":[1,2,3]', '"shell":1,"helper":2,"processes":[1,1,2]'), ('"shell":2,"helper":3,"processes":[1,2,3]', '"shell":2,"helper":2,"processes":[1,2,2]')]:
            with self.assertRaises(ValueError): self.parse(log().replace(old,new))
    def test_exact_owned_recipe(self):
        import copy
        tag='rubix-output-'+'a'*32
        report=dict(tag=tag,containers=[tag+'-first',tag+'-repeat'],runs={name:dict(command=v.run_command(tag,name)) for name in ('first','repeat')})
        v.ownership(report)
        mutations=[]
        for containers in [[],report['containers']+['other'],['other',tag+'-repeat']]:
            changed=copy.deepcopy(report);changed['containers']=containers;mutations.append(changed)
        for argument in ['--privileged','--network=host']:
            changed=copy.deepcopy(report);changed['runs']['first']['command'][5]=argument;mutations.append(changed)
        changed=copy.deepcopy(report);changed['runs']['repeat']['command']=v.run_command(tag,'first');mutations.append(changed)
        for changed in mutations:
            with self.assertRaises(ValueError): v.ownership(changed)
    def test_strict_json(self):
        for text in ['{"x":1,"x":2}','{"x":NaN}','{"x":1e400}']:
            with tempfile.TemporaryDirectory() as tmp:
                path=Path(tmp)/'data';path.write_text(text)
                with self.assertRaises(ValueError): v.load(path)
    def test_frozen_capture(self): v.capture(HERE/'evidence')
if __name__=='__main__': unittest.main()
