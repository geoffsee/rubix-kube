import copy,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import capture,verify

def raw():
    cases='\n'.join('test '+name+' ... ok' for name in sorted(verify.EXPECTED))
    return ('a'*64+'  /decode-tests\n'+cases+'\ntest result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\nRUBIX_NAMESPACE '+json.dumps({'init':1,'shell':7,'helper':8,'processes':[1,7,8]})+'\n').encode()
class Evidence(unittest.TestCase):
    def test_frozen_native_evidence_is_required(self):verify.capture(verify.HERE/'evidence')
    def test_exact_tests_binary_and_namespace(self):
        with tempfile.TemporaryDirectory() as temporary:
            p=Path(temporary)/'raw';p.write_bytes(raw());verify.records(p)
            first=next(iter(sorted(verify.EXPECTED)))
            variants=[raw().replace(first.encode(),b'unknown'),raw().replace(b'14 passed;',b'13 passed;'),raw().replace(b'... ok',b'... FAILED',1),raw().replace(b'  /decode-tests',b'  /other'),raw().replace(b'"helper": 8',b'"helper": 7'),raw().replace(b'[1, 7, 8]',b'[1, 7, 8, 9]')]
            for value in variants:
                p.write_bytes(value)
                with self.assertRaises(ValueError):verify.records(p)
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
