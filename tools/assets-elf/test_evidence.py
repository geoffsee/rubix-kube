import copy,json,pathlib,struct,tempfile,unittest
from unittest.mock import patch
import oracle,qualify,verify
class Evidence(unittest.TestCase):
    def test_independent_minimal_header_and_bad_class(self):
        data=bytearray(120);data[:7]=b'\x7fELF\x02\x01\x01';struct.pack_into('<HHIQQQIHHH',data,16,2,183,1,64,64,0,0,64,56,1)
        value=oracle.inspect(data);self.assertEqual(value['machine'],183);self.assertIsNone(value['interpreter'])
        data[4]=1
        with self.assertRaises(ValueError):oracle.inspect(data)
    def test_strict_json_rejects_duplicates_and_nonfinite(self):
        for raw in ['{"x":1,"x":2}','{"x":NaN}','{"x":1e999}']:
            with self.assertRaises(ValueError):verify.strict(raw)
        with self.assertRaises(ValueError):verify.equal(False,0,'typed')
    def test_cache_corruption_and_symlink_fail_before_network(self):
        with tempfile.TemporaryDirectory() as temporary:
            p=pathlib.Path(temporary)/'asset';p.write_bytes(b'bad')
            with patch('urllib.request.urlopen',side_effect=AssertionError('network')):
                with self.assertRaises(ValueError):qualify.fetch(p,{'sha256':'0'*64})
                p.unlink();p.symlink_to('missing')
                with self.assertRaises(ValueError):qualify.fetch(p,{'sha256':'0'*64})
    def test_source_failure_precedes_output_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            out=pathlib.Path(temporary)/'output'
            with patch.object(qualify,'sources',side_effect=OSError('missing')),patch('sys.argv',['qualify','--cache',temporary,'--output',str(out)]):
                with self.assertRaises(OSError):qualify.main()
            self.assertFalse(out.exists())
    def test_records_reject_missing_duplicate_wrong_target_and_bool_integer(self):
        artifacts={};rows=[]
        for arch,machine in [('amd64',62),('arm64',183)]:
            for role in ['kine','kube-apiserver']:
                expected={'bytes':120,'machine':machine,'elf_type':2,'entry':64,'flags':0,'interpreter':None,'needed':[]}
                artifacts[f'{role}-{arch}']={'oracle':expected,'sha256':'0'*64};rows.append(dict(expected,sha256='0'*64,role=role,architecture=arch,loader='Unknown',loader_relation='Unresolved'))
        def raw(values):return ('\n'.join('RUBIX_ELF '+json.dumps(v) for v in values)+'\ntest result: ok. 1 passed; 0 failed;\n').encode()
        verify.records(raw(rows),artifacts)
        bad=copy.deepcopy(rows);bad[0]['flags']=False
        wrong=copy.deepcopy(rows);wrong[0]['machine']=183
        digest_bad=copy.deepcopy(rows);digest_bad[0]['sha256']='1'*64
        for value in [rows[:3],[rows[0]]*4,bad,wrong,digest_bad]:
            with self.assertRaises(ValueError):verify.records(raw(value),artifacts)
    def test_bounded_read_and_parsed_digest_rejection(self):
        with tempfile.TemporaryDirectory() as temporary:
            p=pathlib.Path(temporary)/'raw';p.write_bytes(b'12345')
            with self.assertRaisesRegex(ValueError,'size limit'):verify.read(p,4)
            p.unlink();p.symlink_to('missing')
            with self.assertRaisesRegex(ValueError,'symlink'):verify.read(p,4)
    def test_published_evidence_is_required(self):
        directory=qualify.HERE/'evidence'
        verify.verify(directory)
        with tempfile.TemporaryDirectory() as temporary:
            target=pathlib.Path(temporary)
            for p in directory.iterdir():
                if p.is_file():(target/p.name).write_bytes(p.read_bytes())
            with (target/'run0.log').open('ab') as stream:stream.write(b'tamper')
            with self.assertRaisesRegex(ValueError,'raw log digest'):verify.verify(target)
if __name__=='__main__':unittest.main()
