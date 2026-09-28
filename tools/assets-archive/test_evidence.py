"""Mutation tests use invented evidence and do not claim a pinned crane capture occurred."""
import base64
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import capture
import graph
import oracle
import runtime
import verify

def header():
    h=bytearray(512)
    h[100:108]=b'0000644\0';h[108:116]=b'0000000\0';h[116:124]=b'0000000\0'
    h[136:148]=b'00000000000\0';h[156]=ord('0');h[257:265]=b'ustar\x0000'
    return bytes(h)

def invented(name):
    # Independent tiny tar/gzip vectors solely for verifier regression testing.
    layers=[];diffs=[]
    for label in ['synthetic-a','synthetic-b']:
        inner=oracle.pack([('fixture.txt',((label+'\n')*1025).encode(),header())])
        diffs.append('sha256:'+oracle.sha(inner));layers.append(oracle.gzip(inner))
    cfg=json.dumps({'os':'linux','architecture':'amd64' if name==oracle.NAMES[0] else 'arm',
                    'rootfs':{'type':'layers','diff_ids':[diffs[0],diffs[1],diffs[0]]}},separators=(',',':')).encode()
    names=[oracle.sha(layer)+'.tar.gz' for layer in layers]
    config_name='sha256:'+oracle.sha(cfg)
    manifest=json.dumps([{'Config':config_name,'Layers':[names[0],names[1],names[0]],
                          'RepoTags':['example.invalid/fixture:'+name]}],separators=(',',':')).encode()
    return oracle.gzip(oracle.pack([(config_name,cfg,header()),*[(n,b,header()) for n,b in zip(names,layers)],('manifest.json',manifest,header())]))

def raw():
    inputs=oracle.cases(invented(oracle.NAMES[0]),invented(oracle.NAMES[1]))
    rows=oracle.observations(inputs)
    lines=['a'*64+'  /producer','b'*64+'  /archive-tests',
           'RUBIX_PRODUCER go1.26.2 github.com/google/go-containerregistry v0.21.5 h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=']
    lines += ['RUBIX_INPUT '+json.dumps({'case':name,'gzip_base64':base64.b64encode(value).decode()}) for name,value in inputs.items()]
    lines += ['test inspect_pinned_crane_serialization ... ok']
    lines += ['RUBIX_ARCHIVE '+json.dumps(row) for row in rows.values()]
    lines += ['test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s',
              'RUBIX_COMPLETE '+json.dumps({'cases':list(oracle.NAMES),'consumer_command':runtime.CONSUMER}),
              'RUBIX_NAMESPACE '+json.dumps({'init':1,'shell':7,'helper':8,'processes':[1,7,8]})]
    return ('\n'.join(lines)+'\n').encode()

def synthetic(directory):
    inventory=verify.current_inventory()
    (directory/'source-inventory.json').write_text(json.dumps(inventory))
    (directory/'build.log').write_text('#10 1.23 '+'a'*64+'  /out/producer\n#10 1.24 '+'b'*64+'  /out/archive-tests\n')
    tag='rubix-archive-'+'f'*32
    report=dict(schema=1,source_revision='0'*40,uncommitted_source_snapshot=False,tag=tag,
                harness_sha256={name:verify.digest(verify.HERE/name) for name in verify.HARNESS},
                helper_sha256=verify.digest(verify.ROOT/'tools/defaults/capture.py'),
                containers=[tag+'-first',tag+'-repeat'],errors=[],cleanup_errors=[],runs={},
                source_inventory_sha256=verify.digest(directory/'source-inventory.json'),
                image_id='sha256:'+'1'*64,remaining_containers=[],remaining_images=[],
                build_log_sha256=verify.digest(directory/'build.log'),
                build_binary_sha256={'producer':'a'*64,'archive-tests':'b'*64})
    for name in ['first','repeat']:
        p=directory/(name+'.log');p.write_bytes(raw());rows,binaries=verify.records(p)
        report['runs'][name]=dict(command=verify.run_command(tag,name),raw_sha256=verify.digest(p),binary_sha256=binaries,records=rows)
    (directory/'receipt.json').write_text(json.dumps(report))
    verify.capture(directory)
    return report

class Evidence(unittest.TestCase):
    def test_invented_observations_and_exact_repeated_reference(self):
        observed=oracle.expected(invented(oracle.NAMES[0]),oracle.NAMES[0])
        self.assertEqual(observed['layers'][0],observed['layers'][2])
        self.assertNotEqual(observed['layers'][0],observed['layers'][1])
        self.assertEqual(oracle.expected(invented(oracle.NAMES[1]),oracle.NAMES[1])['platform_status'],'VariantUnresolved')
    def test_exact_input_output_counts_names_and_producer(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'raw';p.write_bytes(raw());verify.records(p)
            for value in [raw().replace(b'v0.21.5',b'v0.21.4'),raw().replace(b'go1.26.2',b'go1.26.5'),
                          raw().replace(b'... ok',b'... FAILED'),raw().replace(b'1 passed;',b'0 passed;'),
                          raw().replace(b'RUBIX_ARCHIVE ',b'RUBIX_UNKNOWN ',1),raw()+b'RUBIX_COMPLETE {}\n',
                          raw().replace(b'"helper": 8',b'"helper": 7'),raw().replace(b'[1, 7, 8]',b'[1, 7, 8, 9]')]:
                p.write_bytes(value)
                with self.assertRaises(ValueError):verify.records(p)
    def test_equal_substituted_runtime_binaries_fail_builder_binding(self):
        for original_hash in [b'a'*64,b'b'*64]:
            with tempfile.TemporaryDirectory() as tmp:
                directory=Path(tmp);report=synthetic(directory)
                for name in ['first','repeat']:
                    p=directory/(name+'.log');p.write_bytes(raw().replace(original_hash,b'c'*64))
                    rows,binaries=verify.records(p)
                    report['runs'][name].update(raw_sha256=verify.digest(p),records=rows,binary_sha256=binaries)
                (directory/'receipt.json').write_text(json.dumps(report))
                with self.assertRaisesRegex(ValueError,'builder/runtime binary mismatch'):verify.capture(directory)
    def test_builder_records_require_both_actual_unique_digests(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'build';good='#2 0.1 '+'a'*64+'  /out/producer\n#2 0.2 '+'b'*64+'  /out/archive-tests\n'
            p.write_text(good);verify.build_binary(p)
            for bad in ['',good.splitlines()[0],good+good,good.replace('/out/producer','/other'),
                        '#2 RUN sha256sum /out/producer /out/archive-tests\n']:
                p.write_text(bad)
                with self.assertRaises(ValueError):verify.build_binary(p)
    def test_receipt_mutations_reject_privilege_dirty_source_and_false_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);original=synthetic(directory)
            for key,value in [('uncommitted_source_snapshot',True),('cleanup_errors',['failure']),
                              ('remaining_containers',['owned']),('remaining_images',['owned']),
                              ('harness_sha256',{}),('source_inventory_sha256','0'*64),
                              ('build_binary_sha256',{}),('build_log_sha256','0'*64)]:
                report=copy.deepcopy(original);report[key]=value
                (directory/'receipt.json').write_text(json.dumps(report))
                with self.assertRaises(ValueError):verify.capture(directory)
            report=copy.deepcopy(original);report['runs']['first']['command'].insert(2,'--privileged')
            (directory/'receipt.json').write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError,'isolated command'):verify.capture(directory)
    def test_observation_hash_platform_diffid_and_archive_identity_tampering(self):
        for field in ['archive_sha256','config_sha256','archive_manifest_sha256','declared_diff_id','architecture']:
            with tempfile.TemporaryDirectory() as tmp:
                p=Path(tmp)/'raw';lines=raw().decode().splitlines()
                for i,line in enumerate(lines):
                    if line.startswith('RUBIX_ARCHIVE '):
                        row=verify.strict(line.removeprefix('RUBIX_ARCHIVE '))
                        if field=='archive_sha256':row[field]='0'*64
                        elif field=='declared_diff_id':row['observed']['layers'][0][field]='0'*64
                        else:row['observed'][field]='wrong'
                        lines[i]='RUBIX_ARCHIVE '+json.dumps(row);break
                p.write_text('\n'.join(lines)+'\n')
                with self.assertRaisesRegex(ValueError,'independent archive observations'):verify.records(p)
    def test_malformed_or_substituted_encoded_case_fails_independent_derivation(self):
        inputs=oracle.cases(invented(oracle.NAMES[0]),invented(oracle.NAMES[1]))
        for name in oracle.NAMES[2:]:
            changed=dict(inputs);changed[name]=inputs[oracle.NAMES[0]]
            with self.assertRaises(ValueError):oracle.observations(changed)
        for encoded in [inputs[oracle.NAMES[0]][:-1],inputs[oracle.NAMES[0]]+b'junk',inputs[oracle.NAMES[0]]*2]:
            with self.assertRaises(ValueError):oracle.ungzip(encoded)
    def test_stored_layer_or_config_hash_and_declared_diffids_are_independent(self):
        encoded=invented(oracle.NAMES[0]);entries=oracle.unpack(oracle.ungzip(encoded))
        for index in [0,1]:
            changed=list(entries);name,body,h=changed[index];changed[index]=(name,body+b'x',h)
            with self.assertRaises(ValueError):oracle.expected(oracle.gzip(oracle.pack(changed)),oracle.NAMES[0])
        changed=list(entries);name,body,h=changed[0];cfg=oracle.strict(body);cfg['rootfs']['diff_ids'][0]='sha256:'+'0'*64
        body=json.dumps(cfg).encode();new='sha256:'+oracle.sha(body);changed[0]=(new,body,h)
        m=oracle.strict(changed[-1][1]);m[0]['Config']=new;changed[-1]=('manifest.json',json.dumps(m).encode(),changed[-1][2])
        with self.assertRaisesRegex(ValueError,'fixture declared DiffIDs'):oracle.expected(oracle.gzip(oracle.pack(changed)),oracle.NAMES[0])
    def test_complete_graph_cannot_replace_or_omit_checksums(self):
        rows=json.loads((verify.HERE/'modules.json').read_text());raw_graph='\n'.join(json.dumps(row) for row in rows)
        self.assertEqual(graph.normalized(raw_graph),rows)
        for key in ['Sum','GoModSum']:
            changed=copy.deepcopy(rows);del changed[0][key]
            with self.assertRaises(KeyError):graph.normalized('\n'.join(json.dumps(row) for row in changed))
        changed=copy.deepcopy(rows);changed[0]['Replace']={'Path':'evil'}
        with self.assertRaises(ValueError):graph.normalized('\n'.join(json.dumps(row) for row in changed))
    def test_strict_json_bounded_regular_reads(self):
        for text in ['{"x":1,"x":2}','{"x":NaN}','{"x":1.5}']:
            with self.assertRaises(ValueError):verify.strict(text)
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'raw';p.write_bytes(b'12345')
            with self.assertRaises(ValueError):verify.read(p,4)
            p.unlink();p.symlink_to('missing')
            with self.assertRaises(OSError):verify.read(p)
    def test_missing_metadata_prevents_output_creation(self):
        with tempfile.TemporaryDirectory() as tmp:
            out=Path(tmp)/'output'
            with patch.object(capture,'control',side_effect=OSError('missing')),patch('sys.argv',['capture','--output',str(out)]):
                with self.assertRaises(OSError):capture.main()
            self.assertFalse(out.exists())
    def test_dirty_source_prevents_output_and_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            out=Path(tmp)/'output'
            with patch.object(capture,'control',side_effect=['0'*40,' M relevant']),patch('sys.argv',['capture','--output',str(out)]),patch.object(capture.helper,'bounded') as bounded_call:
                with self.assertRaisesRegex(ValueError,'clean source required'):capture.main()
            self.assertFalse(out.exists());bounded_call.assert_not_called()
    def test_failed_cleanup_records_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);report={'containers':['owned'],'cleanup_errors':[]}
            with patch.object(capture.helper.subprocess,'run',side_effect=OSError('daemon unavailable')):
                capture.helper.finish(report,directory,'owned')
            saved=json.loads((directory/'receipt.json').read_text())
            self.assertTrue(saved['cleanup_errors']);self.assertIsNone(saved['remaining_containers']);self.assertIsNone(saved['remaining_images'])

class PublishedEvidence(unittest.TestCase):
    # Required gate; intentionally fails until real clean-revision evidence is published.
    def test_frozen_pinned_crane_evidence_is_required(self):
        verify.capture(verify.HERE/'evidence')

if __name__=='__main__':unittest.main()
