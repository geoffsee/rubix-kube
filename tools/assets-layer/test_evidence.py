"""Mutation tests use invented evidence and do not claim a pinned crane capture occurred."""
import base64
import copy
import contextlib
import io
import types
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

def invented():
    # Invented raw-block zstd and stored-DEFLATE gzip, never real upstream evidence.
    upstream={}; positives={}
    for name in oracle.POSITIVES:
        layers=[]; rows=[]
        for label in ['a','b']:
            raw=oracle.known_raw(label);parts=[raw]
            if name.endswith('-concat'):parts=[raw[:len(raw)//2],raw[len(raw)//2:]]
            if name.startswith('gzip'):stored=b''.join(oracle.gzip(part) for part in parts)
            else:stored=b''.join(b'\x28\xb5\x2f\xfd\x00\x50'+((len(part)<<3)|1).to_bytes(3,'little')+part for part in parts)
            layers.append(stored);rows.append(dict(stored_sha256=oracle.sha(stored),stored_bytes=len(stored),diff_id=oracle.sha(raw),decoded_base64=base64.b64encode(raw).decode(),frames=len(parts),codec=name.split('-')[0]))
        upstream[name]=[rows[0],rows[1],rows[0]]
        cfg=json.dumps({'os':'linux','architecture':'amd64','rootfs':{'type':'layers','diff_ids':['sha256:'+r['diff_id'] for r in upstream[name]]}},separators=(',',':')).encode()
        names=[oracle.sha(layer)+'.tar.gz' for layer in layers];config_name='sha256:'+oracle.sha(cfg)
        manifest=json.dumps([{'Config':config_name,'Layers':[names[0],names[1],names[0]],'RepoTags':['example.invalid/layer:'+name]}],separators=(',',':')).encode()
        positives[name]=oracle.gzip(oracle.pack([(config_name,cfg,header()),*[(n,b,header()) for n,b in zip(names,layers)],('manifest.json',manifest,header())]))
    return positives,upstream

def raw():
    positives,upstream=invented();inputs=oracle.cases(positives,upstream);rows=oracle.observations(inputs,upstream)
    lines=['a'*64+'  /producer','b'*64+'  /layer-tests',
           'RUBIX_PRODUCER go1.26.2 github.com/google/go-containerregistry v0.21.5 h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=',
           'RUBIX_UPSTREAM '+json.dumps(upstream)]
    lines += ['RUBIX_INPUT '+json.dumps({'case':name,'gzip_base64':base64.b64encode(value).decode()}) for name,value in inputs.items()]
    lines += ['test verify_pinned_crane_layers ... ok']
    lines += ['RUBIX_LAYER '+json.dumps(row) for row in rows.values()]
    lines += ['test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s',
              'RUBIX_COMPLETE '+json.dumps({'cases':list(oracle.NAMES),'consumer_command':runtime.CONSUMER}),
              'RUBIX_NAMESPACE '+json.dumps({'init':1,'shell':7,'helper':8,'processes':[1,7,8]})]
    return ('\n'.join(lines)+'\n').encode()

def synthetic(directory):
    inventory=verify.current_inventory()
    (directory/'source-inventory.json').write_text(json.dumps(inventory))
    (directory/'build.log').write_text('#10 1.23 '+'a'*64+'  /out/producer\n#10 1.24 '+'b'*64+'  /out/layer-tests\n')
    tag='rubix-layer-'+'f'*32
    report=dict(schema=1,source_revision='0'*40,uncommitted_source_snapshot=False,tag=tag,
                harness_sha256={name:verify.digest(verify.HERE/name) for name in verify.HARNESS},
                helper_sha256=verify.digest(verify.ROOT/'tools/defaults/capture.py'),
                containers=[tag+'-first',tag+'-repeat'],errors=[],cleanup_errors=[],runs={},
                source_inventory_sha256=verify.digest(directory/'source-inventory.json'),
                image_id='sha256:'+'1'*64,remaining_containers=[],remaining_images=[],
                build_log_sha256=verify.digest(directory/'build.log'),
                build_binary_sha256={'producer':'a'*64,'layer-tests':'b'*64})
    for name in ['first','repeat']:
        p=directory/(name+'.log');p.write_bytes(raw());rows,binaries=verify.records(p)
        report['runs'][name]=dict(command=verify.run_command(tag,name),raw_sha256=verify.digest(p),binary_sha256=binaries,records=rows)
    (directory/'receipt.json').write_text(json.dumps(report))
    verify.capture(directory)
    return report

class Evidence(unittest.TestCase):
    def test_invented_observations_and_exact_repeated_reference(self):
        positives,upstream=invented();oracle.upstream_rows(upstream)
        for name in oracle.POSITIVES:
            observed=oracle.expected(positives[name],name,upstream)
            self.assertEqual(observed['layers'][0],observed['layers'][2])
            self.assertNotEqual(observed['layers'][0],observed['layers'][1])
    def test_exact_input_output_counts_names_and_producer(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'raw';p.write_bytes(raw());verify.records(p)
            for value in [raw().replace(b'v0.21.5',b'v0.21.4'),raw().replace(b'go1.26.2',b'go1.26.5'),
                          raw().replace(b'... ok',b'... FAILED'),raw().replace(b'1 passed;',b'0 passed;'),
                          raw().replace(b'RUBIX_LAYER ',b'RUBIX_UNKNOWN ',1),raw()+b'RUBIX_COMPLETE {}\n',
                          raw().replace(b'"helper": 8',b'"helper": 7'),raw().replace(b'[1, 7, 8]',b'[1, 7, 8, 9]')]:
                p.write_bytes(value)
                with self.assertRaises(ValueError):verify.records(p)
    def test_extra_failed_duplicate_or_unrelated_test_rows_are_rejected(self):
        good = raw()
        expected = b'test verify_pinned_crane_layers ... ok'
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'runtime.log'
            for extra in [b'test unrelated ... FAILED', b'test unrelated ... ok', expected]:
                with self.subTest(extra=extra):
                    path.write_bytes(good.replace(expected, extra+b'\n'+expected))
                    with self.assertRaises(ValueError):
                        verify.records(path)

    def test_consumer_booleans_and_numeric_fields_have_exact_types(self):
        mutations = [
            ('retained_budget', 1), ('retry_retained_budget', 1),
            ('frames', True), ('decoded_bytes', True), ('stored_bytes', True),
            ('outer_bytes', True),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'runtime.log'
            for field, value in mutations:
                with self.subTest(field=field):
                    lines = raw().decode().splitlines()
                    for index, line in enumerate(lines):
                        if line.startswith('RUBIX_LAYER '):
                            row = verify.strict(line.removeprefix('RUBIX_LAYER '))
                            if field in ('retained_budget', 'retry_retained_budget'):
                                row[field] = value
                            elif field == 'outer_bytes':
                                row['observed'][field] = value
                            else:
                                row['observed']['layers'][0][field] = value
                            lines[index] = 'RUBIX_LAYER '+json.dumps(row)
                            break
                    path.write_text('\n'.join(lines)+'\n')
                    with self.assertRaises(ValueError):
                        verify.records(path)

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
    def test_receipt_numeric_boolean_substitutions_cannot_match_valid_raw_records(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            original = synthetic(directory)
            for field in ('retained_budget', 'retry_retained_budget', 'frames'):
                with self.subTest(field=field):
                    report = copy.deepcopy(original)
                    row = report['runs']['first']['records']['gzip']
                    if field == 'frames':
                        row['observed']['layers'][0]['frames'] = True
                    else:
                        row[field] = 1
                    (directory/'receipt.json').write_text(json.dumps(report))
                    with self.assertRaisesRegex(ValueError, 'raw observation binding'):
                        verify.capture(directory)

    def test_builder_records_require_both_actual_unique_digests(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'build';good='#2 0.1 '+'a'*64+'  /out/producer\n#2 0.2 '+'b'*64+'  /out/layer-tests\n'
            p.write_text(good);verify.build_binary(p)
            for bad in ['',good.splitlines()[0],good+good,good.replace('/out/producer','/other'),
                        '#2 RUN sha256sum /out/producer /out/layer-tests\n']:
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
    def test_observation_hash_and_budget_tampering(self):
        for field in ['archive_sha256','outer_sha256','diff_id','decoded_bytes','codec','frames','retained_budget','retry_retained_budget']:
            with tempfile.TemporaryDirectory() as tmp:
                p=Path(tmp)/'raw';lines=raw().decode().splitlines()
                for i,line in enumerate(lines):
                    if line.startswith('RUBIX_LAYER '):
                        row=verify.strict(line.removeprefix('RUBIX_LAYER '))
                        if field in ['retained_budget','retry_retained_budget']:row[field]=False
                        elif field=='archive_sha256':row[field]='0'*64
                        elif field=='outer_sha256':row['observed'][field]='0'*64
                        else:row['observed']['layers'][0][field]='wrong'
                        lines[i]='RUBIX_LAYER '+json.dumps(row);break
                p.write_text('\n'.join(lines)+'\n')
                with self.assertRaises(ValueError):verify.records(p)
    def test_malformed_or_substituted_case_fails_independent_derivation(self):
        positives,upstream=invented();inputs=oracle.cases(positives,upstream)
        for name in oracle.NAMES[4:10]:
            changed=dict(inputs);changed[name]=inputs['gzip']
            with self.assertRaises(ValueError):oracle.observations(changed,upstream)
        for encoded in [inputs['gzip'][:-1],inputs['gzip']+b'junk',inputs['gzip']*2]:
            with self.assertRaises(ValueError):oracle.ungzip(encoded)
        for bad in [dict(list(inputs.items())[:-1]),dict(inputs,extra=inputs['gzip'])]:
            with self.assertRaisesRegex(ValueError,'exact ordered cases'):oracle.observations(bad,upstream)
    def test_wrong_diffid_preserves_repeated_reference_closure_and_stored_hashes(self):
        positives, upstream = invented()
        cases = oracle.cases(positives, upstream)
        before = oracle.unpack(oracle.ungzip(positives['gzip']))
        changed = oracle.unpack(oracle.ungzip(cases['wrong-diffid']))
        self.assertEqual(changed[1:3], before[1:3])
        config = verify.strict(changed[0][1])
        manifest = verify.strict(changed[-1][1])[0]
        declarations = config['rootfs']['diff_ids']
        self.assertEqual(declarations[0], 'sha256:'+'0'*64)
        self.assertEqual(declarations[2], declarations[0])
        self.assertEqual(declarations[1], 'sha256:'+upstream['gzip'][1]['diff_id'])
        self.assertNotEqual(declarations[0], 'sha256:'+upstream['gzip'][0]['diff_id'])
        self.assertEqual(changed[0][0], 'sha256:'+oracle.sha(changed[0][1]))
        self.assertEqual(manifest['Config'], changed[0][0])
        self.assertEqual(manifest['Layers'], [changed[1][0], changed[2][0], changed[1][0]])
        for name, body, _ in changed[1:3]:
            self.assertEqual(name, oracle.sha(body)+'.tar.gz')
        observed = oracle.observations(cases, upstream)['wrong-diffid']['observed']
        self.assertEqual(observed, {'status':'policy:Layer(DiffId)'})

    def test_upstream_raw_bytes_digests_and_member_binding(self):
        positives,upstream=invented()
        for field,value in [('decoded_base64',base64.b64encode(b'wrong').decode()),('diff_id','0'*64),('frames',7),('codec','identity')]:
            changed=copy.deepcopy(upstream);changed['gzip'][0][field]=value;changed['gzip'][2][field]=value
            with self.assertRaises(ValueError):oracle.upstream_rows(changed)
        for index in [0,1]:
            entries=oracle.unpack(oracle.ungzip(positives['gzip']));name,body,h=entries[index];entries[index]=(name,body+b'x',h)
            with self.assertRaises(ValueError):oracle.expected(oracle.gzip(oracle.pack(entries)),'gzip',upstream)
        changed=copy.deepcopy(upstream);changed['zstd'][0]['stored_sha256']='0'*64
        with self.assertRaises(ValueError):oracle.expected(positives['zstd'],'zstd',changed)
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
    def test_failed_consumer_preserves_bounded_output_without_completion(self):
        positives, upstream = invented()
        def producer(command, **kwargs):
            directory = Path(command[1])
            (directory/'upstream.json').write_text(json.dumps(upstream))
            for name, data in positives.items():
                (directory/(name+'.tar.gz')).write_bytes(data)
        for diagnostic in [b'FAILED_CONSUMER_ASSERTION: exact budget mismatch\n',
                           b'FAILED_LOG_START\n'+b'x'*65536+b'FORBIDDEN_OVERSIZE_TAIL', None, 'unreadable']:
            with self.subTest(length=None if diagnostic is None else len(diagnostic)):
                failure = RuntimeError('consumer exited 101')
                def failed_consumer(command, path, deadline, limit):
                    self.assertEqual(command, runtime.CONSUMER)
                    self.assertEqual((deadline, limit), (20, 65536))
                    if diagnostic == 'unreadable':
                        path.mkdir()
                    elif diagnostic is not None:
                        path.write_bytes(diagnostic)
                    raise failure
                output = io.StringIO()
                with patch.object(runtime.subprocess, 'run', side_effect=producer), \
                     patch.object(runtime, 'bounded', types.SimpleNamespace(bounded=failed_consumer)), \
                     patch.dict(runtime.os.environ), contextlib.redirect_stdout(output):
                    with self.assertRaises(RuntimeError) as raised:
                        runtime.main()
                self.assertIs(raised.exception, failure)
                if isinstance(diagnostic, bytes) and len(diagnostic) <= 65536:
                    self.assertTrue(output.getvalue().endswith(diagnostic.decode()))
                elif isinstance(diagnostic, bytes):
                    self.assertNotIn('FAILED_LOG_START', output.getvalue())
                    self.assertIn('Consumer diagnostic exceeded its bound', output.getvalue())
                self.assertNotIn('FORBIDDEN_OVERSIZE_TAIL', output.getvalue())
                self.assertNotIn('RUBIX_COMPLETE ', output.getvalue())

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
