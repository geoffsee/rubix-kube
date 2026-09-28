"""Host-safe verifier regression tests; synthetic receipts are not live qualification evidence."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import support
import verify
import verify_build

# Independent pinned expected behavior, intentionally not derived from verifier constants.
NAMES = ['br_netfilter', 'overlay', 'xt_comment', 'xt_conntrack', 'xt_MASQUERADE',
         'xt_addrtype', 'xt_multiport', 'xt_nat', 'nft_compat', 'nft_numgen',
         'nft_redir', 'nft_limit', 'nft_tproxy']
PATHS = ['/proc/sys/net/ipv6/conf/'+n+'/disable_ipv6' for n in ['all', 'default', 'lo']]
CASES = ['help', 'version', 'print', 'guard_failed', 'double_failed', 'double_limits',
         'double_cancel', 'real_first', 'real_repeat']
TESTS = ['complete_bounded_scalars_reject_prefixes_and_binary_data',
         'owner_launch_failure_has_no_child_and_is_settled',
         'cancellation_before_start_never_observes_or_mutates_network',
         'unjoined_module_owner_stops_all_further_effects',
         'fresh_root_backend_and_ipv4_unknown_guards_prevent_effects',
         'cancellation_after_ipv6_write_preserves_possible_effects_without_readback',
         'cancellation_between_ipv6_read_and_write_prevents_write',
         'cancellation_after_failed_write_retains_possible_effects',
         'ipv6_only_zero_writes_and_success_requires_readback',
         'write_failures_continue_to_all_controls_and_retain_possible_effects',
         'requested_unknown_ipv6_is_deferred_only_by_preparation',
         'fixed_backend_lists_and_external_runtime_keep_network_effects',
         'settled_module_failures_warn_and_continue_without_loaded_claim',
         'cancellation_during_module_waits_for_stop_acknowledgement']

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True)+'\n')

def module_row(name, outcome='Failed', code=17, observed=True):
    return dict(name=name, outcome=outcome, spawned=True, joined=True, reaped=True,
                ownership_lost=False, exit=code,
                after=dict(loaded='Present(false)', builtin_index='Absent',
                           available_index='Unknown(PermissionDenied)') if observed else None)

def results():
    rows={}
    for case in CASES[3:]:
        rows[case]=dict(schema=1, status='Completed', assessment='Observed', runtime='External',
                        family='Present(NfTables)', shared_effects_possible=True,
                        modules=[module_row(n) for n in NAMES],
                        ipv6=[dict(path=p,outcome='AlreadyDisabled' if case=='real_repeat'
                                   else 'ObservedDisabled') for p in PATHS])
    rows['guard_failed'].update(status='GuardStopped', assessment='Unknown',
                                family='Unknown(Io)', shared_effects_possible=False,
                                modules=[], ipv6=[])
    rows['double_limits']['modules'][0].update(outcome='Deadline', exit=None)
    rows['double_limits']['modules'][1].update(outcome='CaptureFailed', exit=None)
    rows['double_cancel'].update(status='Cancelled', ipv6=[],
                                 modules=[module_row(NAMES[0], 'Cancelled', None, False)])
    return rows

def semantic_log(rows=None):
    rows=results() if rows is None else rows
    parts=['SETUP_BEGIN\nSETUP_END\n']
    for case,code in zip(CASES,[0,0,0,1,0,0,1,0,0],strict=True):
        stdout=stderr=''
        if case=='help': stderr='usage: kubesolo\n'
        elif case=='version': stderr=json.dumps(dict(level='info',message='kubesolo version',version='0.1.0'))+'\n'
        elif case=='print': stdout='apiVersion: kubesolo.io/v1alpha1\n'
        else: stdout=json.dumps(rows[case])+'\n'
        parts.append(f'CASE_{case}_BEGIN\nEXIT {code}\nSTDOUT_BEGIN\n{stdout}STDOUT_END\nSTDERR_BEGIN\n{stderr}STDERR_END\nCASE_{case}_END\n')
    for case in ['double_failed','double_limits','double_cancel']:
        names=NAMES[:1] if case=='double_cancel' else NAMES
        parts.append('ATTEMPTS_'+case+'_BEGIN\n'+'\n'.join(names)+'\nATTEMPTS_'+case+'_END\n')
    for case in ['initial','guard_failed','double_cancel','double_failed','double_limits','real_first','real_repeat']:
        value='0' if case in ['initial','guard_failed','double_cancel'] else '1'
        parts.append(f'SCALARS_{case} all={value} default={value} lo={value} \n')
    for name in ['modprobe','iptables']:
        parts.extend([f'ORIGINAL_{name} ABSENT\n', f'RESTORED_{name} ABSENT\n',
                      f'ORIGINAL_KIND_{name} ABSENT\n', f'RESTORED_KIND_{name} ABSENT\n',
                      f'ORIGINAL_RESOLVED_{name} '+ 'a'*64 +f'  /sbin/{name}\n',
                      f'RESTORED_RESOLVED_{name} '+ 'a'*64 +f'  /sbin/{name}\n',
                      f'DOUBLE_{name} {support.digest(support.HERE/(name+"-double.sh"))}  /usr/sbin/{name}\n'])
    parts.extend(marker+'\n' for marker in ['MODULES_BEFORE_BEGIN','MODULES_BEFORE_END',
                                           'MODULES_AFTER_BEGIN','MODULES_AFTER_END',
                                           'EXTERNAL_SENTINEL_UNCHANGED','NETWORK_GUEST_COMPLETE'])
    text=''.join(parts)
    for name,before in [('iptables','double_failed'),('modprobe','real_first')]:
        markers=[line+'\n' for line in text.splitlines() if line.startswith(
            ('RESTORED_'+name+' ', 'RESTORED_KIND_'+name+' ', 'RESTORED_RESOLVED_'+name+' '))]
        for marker in markers:text=text.replace(marker,'')
        text=text.replace('CASE_'+before+'_BEGIN\n',''.join(markers)+'CASE_'+before+'_BEGIN\n')
    return text

def build_log():
    binary=b'synthetic-example-bytes'
    text=f'{sha(binary)}  /out/prepare_host_network\n'+f'{sha(b"synthetic-tests")}  /out/host_network\n'
    text+=''.join('test '+name+' ... ok\n' for name in TESTS)
    text+='test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n'
    for case in ['version','help','print-config']:
        text+=f'CLI_{case}_BEGIN\nsynthetic {case} output\nCLI_{case}_END\n'
    return text.encode()

def build_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True)
    binary=b'synthetic-example-bytes'
    (directory/'prepare_host_network').write_bytes(binary)
    (directory/'build.log').write_bytes(b'synthetic build log\n')
    (directory/'run.log').write_bytes(build_log())
    write_json(directory/'source-hashes.json',verify_build.inventory())
    tag='rubix-node-network-'+'a'*32
    report=dict(schema=1,revision='b'*40,dirty=False,tag=tag,containers=[tag+'-test'],
                command=verify_build.command(tag),errors=[],cleanup_errors=[],remaining_containers=[],
                remaining_images=[],image_id='sha256:'+'c'*64,
                helper_sha256=support.digest(support.ROOT/'tools/defaults/capture.py'),
                source_sha256=support.digest(directory/'source-hashes.json'),
                build_sha256=support.digest(directory/'build.log'),run_sha256=support.digest(directory/'run.log'))
    write_json(directory/'receipt.json',report)
    write_json(directory/'artifact.json',dict(sha256=sha(binary),size=len(binary),
               target='aarch64-unknown-linux-musl',revision='b'*40))
    return report

class StrictReaders(unittest.TestCase):
    def test_publication_metadata_excluded_but_executable_sources_bound(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
                (root/name).write_text('fixture input')
            harness=root/'tools/node-network';harness.mkdir(parents=True)
            names=['capture.py','verify.py','support.py','test_verify.py','build.py',
                   'verify_build.py','guest.sh','modprobe-double.sh','iptables-double.sh',
                   'inputs.json','Build.Dockerfile']
            for name in names:(harness/name).write_text('original')
            with patch.object(support,'ROOT',root):
                before=support.inventory()
                for name in ['README.md','provenance.json']:(harness/name).write_text('published metadata')
                self.assertEqual(support.inventory(),before)
                for name in names:
                    with self.subTest(source=name):
                        (harness/name).write_text('changed executable input')
                        self.assertNotEqual(support.inventory(),before)
                        (harness/name).write_text('original')
                # Scope the documentation exception to this harness only.
                other=root/'crates/example';other.mkdir(parents=True)
                (other/'README.md').write_text('bound crate documentation')
                self.assertNotEqual(support.inventory(),before)

    def test_duplicate_nonfinite_float_and_malformed_json_rejected(self):
        for raw in ['{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}', '{"x":-Infinity}', '{"x":1.5}', '{']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):support.loads(raw)
        self.assertEqual(support.loads('{"x":1}'),{'x':1})

    def test_read_rejects_symlink_directory_and_excess_byte(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);path=root/'data';path.write_bytes(b'12345')
            self.assertEqual(support.read(path,5),b'12345')
            with self.assertRaises(ValueError):support.read(path,4)
            (root/'link').symlink_to(path)
            with self.assertRaises((OSError,ValueError)):support.read(root/'link')
            with self.assertRaises((OSError,ValueError)):support.read(root)

class SemanticMutations(unittest.TestCase):
    def test_valid_synthetic_semantics(self):
        self.assertEqual(verify.semantic(semantic_log())['modules'],NAMES)

    def test_module_omission_reordering_and_duplicate_rejected(self):
        for change in ['omit','reorder','duplicate']:
            rows=results();mods=rows['real_first']['modules']
            if change=='omit':mods.pop()
            elif change=='reorder':mods[0],mods[1]=mods[1],mods[0]
            else:mods[-1]=copy.deepcopy(mods[0])
            with self.subTest(change=change),self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

    def test_readback_wrong_or_missing_rejected(self):
        rows=results();rows['real_first']['ipv6'][0]['outcome']='Readback(Ok(Enabled))'
        with self.assertRaises(ValueError):verify.semantic(semantic_log(rows))
        with self.assertRaises(ValueError):verify.semantic(semantic_log().replace('SCALARS_real_first all=1','SCALARS_real_first all=0'))
        rows=results();rows['real_repeat']['ipv6'].pop()
        with self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

    def test_cancel_later_effects_and_uncertain_cleanup_rejected(self):
        for change in ['extra_module','ipv6','observation','unjoined','lost','no_effect_flag']:
            rows=results();row=rows['double_cancel']
            if change=='extra_module':row['modules'].append(module_row('overlay'))
            elif change=='ipv6':row['ipv6']=[dict(path=PATHS[0],outcome='ObservedDisabled')]
            elif change=='observation':row['modules'][0]['after']=dict(loaded='Present(false)',builtin_index='Absent',available_index='Absent')
            elif change=='unjoined':row['modules'][0]['joined']=False
            elif change=='lost':row['modules'][0]['ownership_lost']=True
            else:row['shared_effects_possible']=False
            with self.subTest(change=change),self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

    def test_guard_effects_and_assumed_backend_rejected(self):
        for field,value in [('modules',[module_row('br_netfilter')]),('ipv6',[dict(path=PATHS[0],outcome='ObservedDisabled')]),('shared_effects_possible',True),('family','Present(NfTables)')]:
            rows=results();rows['guard_failed'][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

    def test_limits_wrong_classification_rejected(self):
        for index in [0,1]:
            rows=results();rows['double_limits']['modules'][index]['outcome']='Failed'
            with self.subTest(index=index),self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

    def test_wrong_restoration_and_double_hash_rejected(self):
        text=semantic_log()
        with self.assertRaises(ValueError):verify.semantic(text.replace('RESTORED_modprobe ABSENT','RESTORED_modprobe '+'a'*64+'  /usr/sbin/modprobe'))
        with self.assertRaises(ValueError):verify.semantic(text.replace('RESTORED_KIND_modprobe ABSENT','RESTORED_KIND_modprobe LINK /bin/busybox'))
        with self.assertRaises(ValueError):verify.semantic(text.replace('RESTORED_RESOLVED_modprobe '+'a'*64,'RESTORED_RESOLVED_modprobe '+'b'*64))
        with self.assertRaises(ValueError):verify.semantic(text.replace('DOUBLE_iptables '+support.digest(support.HERE/'iptables-double.sh'),'DOUBLE_iptables '+'a'*64))

    def test_restoration_after_real_execution_rejected(self):
        text=semantic_log();marker='RESTORED_modprobe ABSENT\n'
        text=text.replace(marker,'')+marker
        with self.assertRaises(ValueError):verify.semantic(text)

    def test_duplicate_cases_timeout_and_boolean_integer_rejected(self):
        text=semantic_log()
        with self.assertRaises(ValueError):verify.semantic(text+text)
        with self.assertRaises(ValueError):verify.semantic(text.replace('CASE_real_first_BEGIN\nEXIT 0','CASE_real_first_BEGIN\nEXIT 124'))
        rows=results();rows['real_first']['modules'][0]['joined']=1
        with self.assertRaises(ValueError):verify.semantic(semantic_log(rows))

class BuildMutations(unittest.TestCase):
    def test_exact_safe_test_inventory_and_cli_completion(self):
        self.assertTrue(verify_build.verify_run(build_log()))
        for raw in [build_log().replace(TESTS[0].encode(),b'unrelated_test'),
                    build_log().replace(TESTS[0].encode(),TESTS[1].encode()),
                    build_log().replace(b'CLI_help_END\n',b''),
                    build_log().replace(b'synthetic help output',b'')]:
            with self.assertRaises(ValueError):verify_build.verify_run(raw)

    def test_build_receipt_source_artifact_and_cleanup_mutations(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);report=build_fixture(root)
            self.assertEqual(verify_build.verify(root)['target'],'aarch64-unknown-linux-musl')
            for key,value in [('dirty',True),('helper_sha256','a'*64),('source_sha256','a'*64),('run_sha256','a'*64),('remaining_containers',['leak']),('remaining_images',['leak']),('cleanup_errors',['failure']),('command',[])]:
                modified=copy.deepcopy(report);modified[key]=value;write_json(root/'receipt.json',modified)
                with self.subTest(key=key),self.assertRaises(ValueError):verify_build.verify(root)
            write_json(root/'receipt.json',report)
            source=support.load(root/'source-hashes.json');source['Cargo.toml']='a'*64
            write_json(root/'source-hashes.json',source);report['source_sha256']=support.digest(root/'source-hashes.json');write_json(root/'receipt.json',report)
            with self.assertRaises(ValueError):verify_build.verify(root)
            build_fixture(root);metadata=support.load(root/'artifact.json');metadata['revision']='d'*40;write_json(root/'artifact.json',metadata)
            with self.assertRaises(ValueError):verify_build.verify(root)
            build_fixture(root);(root/'prepare_host_network').write_bytes(b'changed')
            with self.assertRaises(ValueError):verify_build.verify(root)


def guest_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True)
    build_fixture(directory/'artifact-build')
    metadata=support.load(directory/'artifact-build/artifact.json')
    inputs=support.load(support.HERE/'inputs.json')
    cloud=dict(status='done',errors=[],recoverable_errors={})
    cloud.update({stage:dict(errors=[],start=0.1,finished=0.2) for stage in ['init-local','init','modules-config','modules-final']})
    write_json(directory/'cloud-init.stdout',cloud)
    text=semantic_log();(directory/'network-cases.stdout').write_text(text)
    names={'modprobe-double.sh','iptables-double.sh','prepare_host_network','repo/aarch64/APKINDEX.tar.gz'}
    names.update('repo/aarch64/'+name for name in inputs['packages']['selected'])
    (directory/'verify-guest-inputs.stdout').write_text(''.join(name+': OK\n' for name in sorted(names)))
    (directory/'verify-guest-inputs.stderr').write_bytes(b'')
    private='/tmp/rubix-vm-synthetic';port='127.0.0.1:22222'
    command=['qemu-system-aarch64','-machine','virt,accel=hvf','-cpu','host','-smp','2','-m','2048',
      '-display','none','-serial','stdio','-monitor','none','-qmp','unix:'+private+'/qmp.sock,server=on,wait=off',
      '-drive','if=pflash,format=raw,readonly=on,file=/opt/homebrew/share/qemu/edk2-aarch64-code.fd',
      '-drive','if=pflash,format=raw,file='+private+'/vars.fd','-drive','if=virtio,format=qcow2,file='+private+'/disk.qcow2',
      '-drive','if=virtio,format=raw,readonly=on,file='+private+'/seed.iso','-netdev',
      'user,id=n0,restrict=on,hostfwd=tcp:'+port+'-:22','-device','virtio-net-pci,netdev=n0']
    sources=['capture.py','inputs.json','guest.sh','verify.py','support.py','modprobe-double.sh','iptables-double.sh']
    report=dict(status='passed',errors=[],owned_process_group_absent=True,
                owned_temporary_directory_removed=True,explicit_privilege_opt_in=True,
                qemu_exit_code=0,shutdown='guest-poweroff',privileged_mount_probe='passed',
                revision='b'*40,working_tree_snapshot=False,adapter='qemu-disposable-node-network',
                source_sha256={name:support.digest(support.HERE/name) for name in sources},
                baseline_helper_sha256=support.digest(support.ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),
                inherited_vm_sha256=support.digest(support.ROOT/'tools/parity/vm/run.py'),
                build_verifier_sha256=support.digest(support.HERE/'verify_build.py'),
                build_support_sha256=support.digest(support.HERE/'support.py'),
                inputs=inputs,tools={name:dict(sha256=value) for name,value in inputs['host_tools'].items()},
                firmware=inputs['firmware'],artifact_sha256=metadata['sha256'],cloud_init=cloud,cloud_init_exit=0,
                serial_log_truncated=False,owned_temporary_directory=private,ssh_forward=port,
                qemu_argv=command,guest_host_public_key='ssh-ed25519 YWJjZA== rubix-alpine-fixture-host',
                observation=text)
    write_json(directory/'result.json',report)
    return report

class GuestMutations(unittest.TestCase):
    def test_guest_receipt_binding_ownership_and_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);report=guest_fixture(root)
            self.assertEqual(verify.verify_guest(root)['modules'],NAMES)
            for key,value in [('working_tree_snapshot',True),('errors',['failure']),
                              ('owned_process_group_absent',False),('owned_temporary_directory_removed',False),
                              ('qemu_exit_code',True),('qemu_exit_code',1),('shutdown','forced-process-termination'),
                              ('artifact_sha256','f'*64),('build_verifier_sha256','f'*64),
                              ('source_sha256',{}),('qemu_argv',[]),('ssh_forward','0.0.0.0:22222'),
                              ('cloud_init_exit',True),('serial_log_truncated',True),('observation','forged')]:
                changed=copy.deepcopy(report);changed[key]=value;write_json(root/'result.json',changed)
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):verify.verify_guest(root)
            write_json(root/'result.json',report)
            (root/'verify-guest-inputs.stdout').write_text('prepare_host_network: OK\n')
            with self.assertRaises(ValueError):verify.verify_guest(root)

    def test_guest_raw_cloud_and_candidate_records_are_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);report=guest_fixture(root)
            cloud=copy.deepcopy(report['cloud_init']);cloud['init']['errors']=['failed']
            write_json(root/'cloud-init.stdout',cloud)
            with self.assertRaises(ValueError):verify.verify_guest(root)
            report['cloud_init']=cloud;write_json(root/'result.json',report)
            with self.assertRaises(ValueError):verify.verify_guest(root)
            guest_fixture(root)
            (root/'verify-guest-inputs.stderr').write_text('sha256sum: mismatched input\n')
            with self.assertRaises(ValueError):verify.verify_guest(root)

    def test_vm_reader_rejects_nonfinite_numbers_and_duplicate_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'cloud.json'
            for value in ['{"time":1e999}','{"time":NaN}','{"time":1,"time":2}']:
                path.write_text(value)
                with self.subTest(value=value),self.assertRaises(ValueError):verify.vm_load(path)
            path.write_text('{"time":0.125}')
            self.assertEqual(verify.vm_load(path),{'time':0.125})

    def test_published_raw_inventory_and_secret_rejections(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);evidence=root/'evidence';evidence.mkdir()
            (evidence/'unexpected').write_bytes(b'raw')
            write_json(root/'provenance.json',{'files':{}})
            with self.assertRaises(ValueError):verify.verify(root)
            write_json(root/'provenance.json',{'files':{'unexpected':'f'*64}})
            with self.assertRaises(ValueError):verify.verify(root)
            (evidence/'unexpected').write_bytes(b'BEGIN OPENSSH PRIVATE KEY')
            write_json(root/'provenance.json',{'files':{'unexpected':support.digest(evidence/'unexpected')}})
            with self.assertRaises(ValueError):verify.verify(root)


class FrozenEvidence(unittest.TestCase):
    """Required real publication checks: absent or stale evidence is a failure, never a skip."""
    def test_both_frozen_linux_build_receipts(self):
        for name in ['first', 'repeat']:
            with self.subTest(guest=name):
                verify_build.verify(support.HERE/'evidence'/name/'artifact-build', binary=False)

    def test_first_frozen_guest(self):
        verify.verify_guest(support.HERE/'evidence/first')

    def test_repeat_frozen_guest(self):
        verify.verify_guest(support.HERE/'evidence/repeat')

    def test_complete_hashed_publication(self):
        verify.verify(support.HERE)

if __name__=='__main__':unittest.main()
