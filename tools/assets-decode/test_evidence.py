import copy,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import capture,verify

def raw():
    lines=['a'*64+'  /decode-tests','b'*64+'  /decoded_elf-tests']
    for suite,cases in verify.EXPECTED.items():
        lines.append('RUBIX_SUITE '+suite)
        lines.extend('test '+name+' ... ok' for name in sorted(cases))
        lines.append('test result: ok. '+str(len(cases))+' passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s')
    lines.append('RUBIX_NAMESPACE '+json.dumps({'init':1,'shell':7,'helper':8,'processes':[1,7,8]}))
    return ('\n'.join(lines)+'\n').encode()
def synthetic(directory):
    inventory=verify.current_inventory()
    (directory/'source-inventory.json').write_text(json.dumps(inventory))
    tag='rubix-decode-'+'f'*32
    report=dict(schema=2,source_revision='0'*40,uncommitted_source_snapshot=False,tag=tag,
        harness_sha256={name:verify.digest(verify.HERE/name) for name in verify.HARNESS},
        helper_sha256=verify.digest(verify.ROOT/'tools/defaults/capture.py'),
        containers=[tag+'-first',tag+'-repeat'],errors=[],cleanup_errors=[],runs={},
        source_inventory_sha256=verify.digest(directory/'source-inventory.json'),
        image_id='sha256:'+'1'*64,remaining_containers=[],remaining_images=[])
    for name in ['first','repeat']:
        path=directory/(name+'.log');path.write_bytes(raw());cases,binaries=verify.records(path)
        report['runs'][name]=dict(command=verify.run_command(tag,name),raw_sha256=verify.digest(path),binary_sha256=binaries,records=cases)
    (directory/'receipt.json').write_text(json.dumps(report))
    verify.capture(directory)
    return report
class Evidence(unittest.TestCase):
    def test_frozen_native_evidence_is_required(self):verify.capture(verify.HERE/'evidence')
    def test_exact_tests_binary_and_namespace(self):
        with tempfile.TemporaryDirectory() as temporary:
            p=Path(temporary)/'raw';p.write_bytes(raw());verify.records(p)
            first=next(iter(sorted(verify.DECODE_CASES)))
            variants=[raw().replace(first.encode(),b'unknown'),raw().replace(b'14 passed;',b'13 passed;'),raw().replace(b'... ok',b'... FAILED',1),raw().replace(b'  /decode-tests',b'  /other'),raw().replace(b'"helper": 8',b'"helper": 7'),raw().replace(b'[1, 7, 8]',b'[1, 7, 8, 9]')]
            for value in variants:
                p.write_bytes(value)
                with self.assertRaises(ValueError):verify.records(p)
    def test_both_suites_and_binary_identities_are_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'raw'
            first=sorted(verify.DECODE_CASES)[0].encode()
            second=sorted(verify.ELF_CASES)[0].encode()
            variants=[
                raw().replace(b'RUBIX_SUITE decoded_elf',b'RUBIX_SUITE decode'),
                raw().replace(b'RUBIX_SUITE decode\n',b''),
                raw().replace(b'10 passed;',b'9 passed;'),
                raw().replace(b'b'*64+b'  /decoded_elf-tests\n',b''),
                raw().replace(b'b'*64+b'  /decoded_elf-tests',b'b'*64+b'  /decode-tests'),
                raw().replace(first,b'temporary').replace(second,first).replace(b'temporary',second),
                raw().replace(b'test '+second+b' ... ok',b'test '+second+b' ... ignored'),
                raw()+b'RUBIX_SUITE decoded_elf\n',
            ]
            for value in variants:
                path.write_bytes(value)
                with self.assertRaises(ValueError):verify.records(path)
    def test_rehashed_repeat_binary_substitution_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary);report=synthetic(directory)
            path=directory/'repeat.log';path.write_bytes(raw().replace(b'b'*64,b'c'*64))
            cases,binaries=verify.records(path)
            report['runs']['repeat'].update(raw_sha256=verify.digest(path),records=cases,binary_sha256=binaries)
            (directory/'receipt.json').write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError,'repeated executables'):verify.capture(directory)
    def test_wrong_suite_record_and_command_cannot_be_relabelled(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary);original=synthetic(directory)
            for mutation in ['records','hash','command']:
                report=copy.deepcopy(original);run=report['runs']['first']
                if mutation=='records':run['records']['decoded_elf']=run['records']['decode']
                elif mutation=='hash':run['binary_sha256']['decoded_elf']=run['binary_sha256']['decode']
                else:run['command'][-1]=run['command'][-1].replace('/decoded_elf-tests --nocapture','/decode-tests --nocapture')
                (directory/'receipt.json').write_text(json.dumps(report))
                with self.assertRaises(ValueError):verify.capture(directory)
    def test_strict_json_and_bounded_regular_reads(self):
        for value in ['{"x":1,"x":2}','{"x":NaN}','{"x":1e999}']:
            with self.assertRaises(ValueError):verify.strict(value)
        with tempfile.TemporaryDirectory() as temporary:
            p=Path(temporary)/'raw';p.write_bytes(b'12345')
            with self.assertRaises(ValueError):verify.read(p,4)
            p.unlink();p.symlink_to('missing')
            with self.assertRaises(OSError):verify.read(p)
    def test_missing_metadata_fails_before_output_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            out=Path(temporary)/'output'
            with patch.object(capture,'control',side_effect=OSError('missing')),patch('sys.argv',['capture','--output',str(out)]):
                with self.assertRaises(OSError):capture.main()
            self.assertFalse(out.exists())
    def test_daemon_cleanup_failure_preserves_failed_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary);report={'containers':['owned'],'cleanup_errors':[]}
            with patch.object(capture.helper.subprocess,'run',side_effect=OSError('daemon unavailable')):capture.helper.finish(report,output,'owned')
            saved=json.loads((output/'receipt.json').read_text());self.assertTrue(saved['cleanup_errors']);self.assertIsNone(saved['remaining_containers']);self.assertIsNone(saved['remaining_images'])
    def test_stored_tampering_cannot_pass(self):
        source=verify.HERE/'evidence'
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)
            for p in source.iterdir():
                if p.is_file():(output/p.name).write_bytes(p.read_bytes())
            original=verify.load(output/'receipt.json')
            for key,value in [('cleanup_errors',['failed']),('remaining_containers',['owned']),('harness_sha256',{}),('source_inventory_sha256','0'*64)]:
                report=copy.deepcopy(original);report[key]=value;(output/'receipt.json').write_text(json.dumps(report))
                with self.assertRaises(ValueError):verify.capture(output)
            (output/'receipt.json').write_text(json.dumps(original));(output/'first.log').write_bytes(raw())
            with self.assertRaises(ValueError):verify.capture(output)
    def test_privileged_extra_argument_is_rejected(self):
        source=verify.HERE/'evidence'
        with tempfile.TemporaryDirectory() as temporary:
            output=Path(temporary)
            for p in source.iterdir():
                if p.is_file():(output/p.name).write_bytes(p.read_bytes())
            report=verify.load(output/'receipt.json');report['runs']['first']['command'].insert(2,'--privileged');(output/'receipt.json').write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError,'isolated command'):verify.capture(output)
if __name__=='__main__':unittest.main()
