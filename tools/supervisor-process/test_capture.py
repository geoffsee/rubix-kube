import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
import capture
import verify

class Capture(unittest.TestCase):
    def test_strict_json(self):
        for text in [b'{"a":1,"a":2}',b'NaN',b'1e400',b'-Infinity']:
            with self.assertRaises(ValueError):verify.strict(text)
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'huge';p.write_bytes(b' '*(2*verify.LIMIT+1))
            with self.assertRaises(ValueError):verify.load(p)

    def test_subprocess_output_and_deadline_are_bounded(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'log'
            with self.assertRaisesRegex(ValueError,'output exceeded'):
                capture.run([sys.executable,'-c','print("x"*2097152)'],5,p)
            self.assertLessEqual(p.stat().st_size,verify.LIMIT)
            with self.assertRaises(TimeoutError):
                capture.run([sys.executable,'-c','import time; time.sleep(30)'],.1,p)

    def test_semantic_negatives(self):
        rows=[]
        for name in verify.CASES:
            graceful=name in ['delayed','family','term-error','sentinel-survived-external-stop']
            code=17 if name in ['early','leader-exits-first','term-error'] else 0 if graceful or name=='oneshot' else None
            rows.append(dict(case=name,spawned=True,term_attempted=graceful or name=='ignore',kill_attempted=True,
                             leader_reaped=True,thread_joined=True,complete=True,exit_code=code,signal=None if code is not None else 9,
                             elapsed_ms=30000 if name=='ignore' else 0))
        verify.verify_records(rows)
        for field,value in [('leader_reaped',False),('thread_joined',False),('complete',1),('term_attempted',False),('exit_code',17)]:
            changed=copy.deepcopy(rows);changed[0][field]=value
            with self.assertRaises(ValueError):verify.verify_records(changed)
        changed=copy.deepcopy(rows);changed[2]['elapsed_ms']=100
        with self.assertRaises(ValueError):verify.verify_records(changed)
        with self.assertRaises(ValueError):verify.verify_records(rows[:-1])
        changed=copy.deepcopy(rows);changed[1]=changed[0]
        with self.assertRaises(ValueError):verify.verify_records(changed)



    def staged(self, directory):
        # Synthetic envelope isolates verifier failures; real frozen captures remain separate evidence.
        records=[]
        for name in verify.CASES:
            graceful=name in ['delayed','family','term-error','sentinel-survived-external-stop']
            code=17 if name in ['early','leader-exits-first','term-error'] else 0 if graceful or name=='oneshot' else None
            records.append(dict(case=name,spawned=True,term_attempted=graceful or name=='ignore',kill_attempted=True,
                                leader_reaped=True,thread_joined=True,complete=True,exit_code=code,signal=None if code is not None else 9,
                                elapsed_ms=30000 if name=='ignore' else 0))
        image='rubix-process-'+'a'*32
        receipt=dict(schema=1,source_revision='a'*40,uncommitted_source_snapshot=True,
                     source_sha256=verify.source_inventory(),driver_sha256=verify.digest(verify.HERE/'capture.py'),
                     verifier_sha256=verify.digest(verify.HERE/'verify.py'),runs={},cleanup_errors=[],remaining_containers=[],remaining_images=[],
                     build_command=['docker','build','--platform=linux/arm64','--tag',image,'--file','/tmp/context/tools/supervisor-process/Capture.Dockerfile','/tmp/context'],
                     build_exit_code=0,image_id='sha256:'+'b'*64,repeat_equal=True)
        (directory/'source-inventory.json').write_text(json.dumps(receipt['source_sha256'],sort_keys=True,indent=2)+'\n')
        receipt['source_inventory_sha256']=verify.digest(directory/'source-inventory.json')
        for name in ['first','repeat']:
            raw=('0'*64+'  /process-tests\n'+''.join('RUBIX_PROCESS '+json.dumps(row)+'\n' for row in records)+
                 'RUBIX_NAMESPACE '+json.dumps(dict(init=1,shell=7,helper=8,processes=[1,7,8]))+'\n')
            (directory/(name+'.log')).write_text(raw)
            (directory/(name+'.json')).write_text(json.dumps(records))
            command=['docker','run','--name',image+'-'+name,'--init','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=96','--memory=512m','--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=64m','--env','RUBIX_PROCESS_DISPOSABLE=1',image,'/bin/sh','-c','sha256sum /process-tests; /process-tests --ignored --exact disposable_process_cases --nocapture; status=$?; /usr/local/bin/python3 /namespace_inventory.py || exit $?; exit "$status"']
            receipt['runs'][name]=dict(exit_code=0,raw_sha256=verify.digest(directory/(name+'.log')),command=command,binary_sha256='0'*64)
        return receipt

    def test_receipt_mutations_fail_in_optimized_cli(self):
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);valid=self.staged(directory);path=directory/'receipt.json'
            path.write_text(json.dumps(valid));verify.verify_capture(directory)
            mutations=[]
            for key in ['driver_sha256','source_sha256','runs']:
                value=copy.deepcopy(valid);del value[key];mutations.append(value)
            for key,value in [('build_exit_code',False),('cleanup_errors',['failed']),('remaining_images',None),('driver_sha256','0'*64),('capture_error','failed')]:
                changed=copy.deepcopy(valid);changed[key]=value;mutations.append(changed)
            changed=copy.deepcopy(valid);changed['runs']['first']['exit_code']=1;mutations.append(changed)
            changed=copy.deepcopy(valid);del changed['runs']['repeat'];mutations.append(changed)
            changed=copy.deepcopy(valid);changed['source_sha256'].pop(next(iter(changed['source_sha256'])));mutations.append(changed)
            changed=copy.deepcopy(valid);changed['runs']['first']['command'].remove('--init');mutations.append(changed)
            for value in mutations:
                path.write_text(json.dumps(value))
                result=subprocess.run([sys.executable,'-O',str(verify.HERE/'verify.py'),str(directory)],capture_output=True,timeout=10)
                self.assertNotEqual(result.returncode,0)
            path.write_text(json.dumps(valid));(directory/'first.json').write_text('[]')
            with self.assertRaisesRegex(ValueError,'raw records'):verify.verify_capture(directory)

    def test_duplicate_records_and_namespace_residue_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);self.staged(directory);path=directory/'first.log';original=path.read_text()
            for text in [original.replace('"leader_reaped": true','"leader_reaped": false, "leader_reaped": true',1),
                         original.replace('"processes": [1, 7, 8]','"processes": [1, 7, 8, 9]'),
                         original+'RUBIX_NAMESPACE {}\n']:
                path.write_text(text)
                with self.assertRaises(ValueError):verify.raw_records(path)

    def test_setup_failure_still_records_unknown_cleanup(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'capture'
            with patch.object(sys,'argv',['capture.py','--output',str(directory)]),patch.object(capture,'short',side_effect=RuntimeError('daemon unavailable')):
                with self.assertRaises(RuntimeError):capture.main()
            receipt=verify.load(directory/'receipt.json')
            self.assertIsNone(receipt['remaining_containers']);self.assertIsNone(receipt['remaining_images'])
            self.assertEqual(len(receipt['cleanup_errors']),3)
            self.assertIn('capture_error',receipt)

    def test_frozen_process_evidence(self):
        verify.verify_capture(verify.HERE/'evidence',current=False)

    def test_relevant_mode_binds_historical_inventory_and_current_process_inputs(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);receipt=self.staged(directory)
            (directory/'receipt.json').write_text(json.dumps(receipt))
            original=verify.source_inventory()
            unrelated=dict(original);unrelated['crates/rubix-config/new-persistence.rs']='0'*64
            with patch.object(verify,'source_inventory',return_value=unrelated):
                verify.verify_capture(directory,current=False)
                with self.assertRaisesRegex(ValueError,'complete current'):verify.verify_capture(directory)
            for name in ['Cargo.lock','crates/rubix-supervisor/src/process.rs','tools/supervisor-process/fixture.py']:
                changed=dict(original);changed[name]='0'*64
                with patch.object(verify,'source_inventory',return_value=changed):
                    with self.assertRaisesRegex(ValueError,'relevant source'):verify.verify_capture(directory,current=False)
            missing=dict(original);del missing['crates/rubix-supervisor/tests/process_ownership.rs']
            with patch.object(verify,'source_inventory',return_value=missing):
                with self.assertRaisesRegex(ValueError,'relevant source'):verify.verify_capture(directory,current=False)
            changed=copy.deepcopy(receipt)
            unrelated_key=next(k for k in changed['source_sha256'] if not verify.relevant_source(k))
            del changed['source_sha256'][unrelated_key]
            (directory/'receipt.json').write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError,'historical inventory'):verify.verify_capture(directory,current=False)

    def test_frozen_mutations_fail_without_relaxing_source_checks(self):
        import shutil
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'evidence';shutil.copytree(verify.HERE/'evidence',directory)
            receipt=verify.load(directory/'receipt.json')
            (directory/'first.log').write_bytes((directory/'first.log').read_bytes()+b'changed diagnostics')
            with self.assertRaisesRegex(ValueError,'raw digest'):verify.verify_capture(directory,current=False)
            shutil.copyfile(verify.HERE/'evidence/first.log',directory/'first.log')
            receipt['runs']['repeat']['exit_code']=1
            (directory/'receipt.json').write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError,'run exit'):verify.verify_capture(directory,current=False)

if __name__=='__main__':unittest.main()
