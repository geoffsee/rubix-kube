import copy
import unittest
import tempfile
import json
import sys
import hashlib
from unittest.mock import patch
import capture
import verify

class Semantics(unittest.TestCase):
    def test_reference(self):
        verify.verify(verify.load(verify.HERE / 'reference.json'))

    def test_mutations(self):
        def change(case, key, value):
            rows = copy.deepcopy(verify.expected())
            rows[case][key] = value
            return rows
        mutations = [change(0, 'destination_inode_preserved', True),
                     change(2, 'backup_other_same_inode', False),
                     change(4, 'write_error', False), change(5, 'write_error', 1),
                     verify.expected()[:-1]]
        for index, name, key, value in [(0,'other','bytes','corrupted'),
                                       (1,'other','bytes','unrelated sentinel\n'),
                                       (1,'config.yaml.bak','kind','regular'),
                                       (3,'config.yaml.bak','mode','0600'),
                                       (5,'config.yaml','bytes','overwritten')]:
            rows=copy.deepcopy(verify.expected());rows[index]['entries'][name][key]=value;mutations.append(rows)
        rows=copy.deepcopy(verify.expected());rows[6]['entries']['leaked.tmp']={};mutations.append(rows)
        for rows in mutations:
            with self.subTest(rows=rows):
                with self.assertRaises(ValueError):verify.verify(rows)

    def test_cleanup_failure_still_publishes_unknown_inventory(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = verify.Path(tmp)
            report = {'cleanup_errors': []}
            with patch.object(capture, 'run', side_effect=RuntimeError('daemon unavailable')), patch.object(capture.subprocess, 'check_output', side_effect=RuntimeError('daemon unavailable')):
                capture.cleanup_and_receipt(report, 'owned-only', ['owned-only-first'], output)
            receipt = verify.load(output / 'receipt.json')
            self.assertEqual(len(receipt['cleanup_errors']), 4)
            self.assertIsNone(receipt['remaining_containers'])
            self.assertIsNone(receipt['remaining_images'])

    def test_evidence_binding(self):
        verify.verify_frozen()

    def stage_capture(self, directory):
        import shutil
        root = verify.HERE
        for run, source in [('first', root / 'reference.json'),
                            ('repeat', root / 'evidence/repeat.json')]:
            shutil.copyfile(source, directory / (run + '.json'))
            shutil.copyfile(root / 'evidence' / (run + '.log'), directory / (run + '.log'))
        receipt = verify.load(root / 'capture-receipt.json')
        for key, name in [('capture_driver_sha256', 'capture.py'),
                          ('verifier_sha256', 'verify.py')]:
            receipt[key] = verify.digest(root / name)
        return receipt

    def test_complete_capture_and_receipt_mutations(self):
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            directory = verify.Path(tmp)
            valid = self.stage_capture(directory)
            path = directory / 'receipt.json'
            path.write_text(json.dumps(valid))
            verify.verify_capture(directory)
            mutations = []
            for key in ['capture_driver_sha256', 'runs', 'reference_revision']:
                item = copy.deepcopy(valid); del item[key]; mutations.append(item)
            for key, value in [('capture_error', 'failed'), ('schema', True),
                               ('capture_driver_sha256', '0' * 64),
                               ('reference_revision', '0' * 40), ('remaining_images', None)]:
                item = copy.deepcopy(valid); item[key] = value; mutations.append(item)
            for name in ['first', 'repeat']:
                item = copy.deepcopy(valid); item['runs'][name]['exit_code'] = 1; mutations.append(item)
            item = copy.deepcopy(valid); del item['runs']['repeat']; mutations.append(item)
            item = copy.deepcopy(valid); item['runs']['first']['exit_code'] = False; mutations.append(item)
            item = copy.deepcopy(valid); item['harness_sha256'] = {}; mutations.append(item)
            for item in mutations:
                path.write_text(json.dumps(item))
                with self.assertRaises(ValueError): verify.verify_capture(directory)
                result = subprocess.run([sys.executable, '-O', str(verify.HERE / 'verify.py'), str(directory)],
                                        capture_output=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
            path.write_text(json.dumps(valid))
            (directory / 'first.json').write_text('[]')
            with self.assertRaisesRegex(ValueError, 'raw to normalized'): verify.verify_capture(directory)

    def test_raw_log_binding_and_duplicate_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = verify.Path(tmp)
            receipt = self.stage_capture(directory)
            log = directory / 'first.log'
            original = log.read_bytes()
            log.write_bytes(original + b'changed diagnostics')
            (directory / 'receipt.json').write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, 'raw digest'): verify.verify_capture(directory)
            duplicate = original.replace(b'"write_error":false', b'"write_error":true,"write_error":false', 1)
            self.assertNotEqual(duplicate, original)
            log.write_bytes(duplicate)
            receipt['runs']['first']['stdout_sha256'] = verify.digest(log)
            (directory / 'receipt.json').write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, 'duplicate JSON key'): verify.verify_capture(directory)

    def test_missing_local_and_source_hash_entries(self):
        original_load = verify.load
        for inventory in ['local_sha256', 'reference_source_sha256']:
            provenance = original_load(verify.HERE / 'provenance.json')
            provenance[inventory].pop(next(iter(provenance[inventory])))
            def changed_load(path):
                if path == verify.HERE / 'provenance.json': return provenance
                return original_load(path)
            with patch.object(verify, 'load', side_effect=changed_load):
                with self.assertRaises(ValueError): verify.verify_frozen()

    def test_output_budget(self):
        with tempfile.TemporaryFile() as log:
            with self.assertRaisesRegex(ValueError, 'output exceeds'):
                capture.run([sys.executable, '-c', 'import sys; sys.stdout.write("x" * 2097152)'], 5, stdout=log, stderr=capture.subprocess.STDOUT)
            self.assertLessEqual(log.tell(), 1048576)

    def test_strict_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = verify.Path(tmp) / 'bad.json'
            for text in ['{"a":1,"a":2}', '{"a":NaN}', '{"a":Infinity}', '{"a":1e400}', '{"a":-1e400}']:
                path.write_text(text)
                with self.assertRaises(ValueError): verify.load(path)
            path.write_bytes(b' ' * (verify.LIMIT + 1))
            with self.assertRaisesRegex(ValueError, 'byte budget'): verify.load(path)

if __name__ == '__main__': unittest.main()
