"""Independent host-safe mutation tests. Invented records are never VM evidence."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import support
import build
import verify
import verify_build

BINARIES = ['prepare_node_host', 'rubix_kube', 'host_preparation', 'host_network']
CONTROLLERS = ['cpu', 'cpuset', 'io', 'memory', 'pids']
MODULES = ['br_netfilter', 'overlay', 'xt_comment', 'xt_conntrack', 'xt_MASQUERADE',
           'xt_addrtype', 'xt_multiport', 'xt_nat', 'nft_compat', 'nft_numgen',
           'nft_redir', 'nft_limit', 'nft_tproxy']

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True)+'\n')

def frame(name, body):
    return name+'_BEGIN\n'+body+name+'_END\n'

def config_text():
    return ('api:\n  enabled: false\napiVersion: kubesolo.io/v1alpha1\nkind: Config\n'
            'network:\n  disableIPv6: false\nruntime:\n  containerMode: true\n'
            '  endpoint: unix:///tmp/external-runtime/containerd.sock\n')

def metadata():
    return dict(target='aarch64-unknown-linux-musl', revision='a'*40,
                files={name:dict(sha256=sha(name.encode()),size=len(name)) for name in BINARIES})

def test_log():
    # Pin file is reviewed fixture input, not execution evidence; invented test lines.
    inventory=support.load(support.HERE/'test-inventory.json')
    return ''.join(frame('TEST_'+binary,''.join('test '+name+' ... ok\n' for name in names)+
                        f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n')
                   for binary,names in inventory.items())

def mounts(shared=False):
    flag=' shared:1' if shared else ''
    return (f'1 0 8:1 / / rw{flag} - ext4 /dev/root rw\n'
            f'2 1 0:2 / /proc rw{flag} - proc proc rw\n'
            f'3 1 0:3 /rubix-container-real /sys/fs/cgroup rw{flag} - cgroup2 cgroup rw\n')

def observation(phase):
    ready=phase.endswith('READY');scenario=phase.split('_')[0];pid=701 if scenario=='signal' else 702
    vals={'PID':str(pid),'EXE':metadata()['files']['prepare_node_host']['sha256']+f'  /proc/{pid}/exe',
          'MNT_NS':'mnt:[201]','CGROUP_NS':'cgroup:[202]','OBSERVER_MNT_NS':'mnt:[101]',
          'OBSERVER_CGROUP_NS':'cgroup:[102]','GLOBAL_ROOT_ID':'4:1','ROOT_ID':'4:2','VISIBLE_ROOT_ID':'4:2'}
    text=''.join(k+' '+v+'\n' for k,v in vals.items())
    values={'STAT':f'{pid} (fixture (name)) S '+' '.join(['0']*18+['12345'])+'\n',
            'CANDIDATE_MOUNTS':mounts(not ready),'OBSERVER_MOUNTS':mounts(),
            'PROCESS_CGROUP':'0::/rubix-container-'+scenario+('' if ready else '/init')+'\n',
            'ROOT_TYPE':'domain\n','ROOT_MEMBERS':str(pid)+'\n' if ready else '',
            'AVAILABLE':' '.join(CONTROLLERS)+'\n','ROOT_ENABLED':'' if ready else ' '.join(CONTROLLERS)+'\n',
            'PARENT_ENABLED':' '.join(CONTROLLERS)+'\n',
            'SIBLING':''.join('a'*64+'  /sys/fs/cgroup/rubix-container-sibling/'+name+'\n'
                              for name in ['cgroup.type','cgroup.procs','cgroup.subtree_control','pids.max']),
            'EXTERNAL':''.join(sha(body.encode())+'  /tmp/external-runtime/'+name+'\n'
                               for name,body in [('config.toml','host-owned configuration sentinel\n'),('state','host-owned state sentinel\n')])}
    text+=''.join(frame(k,v) for k,v in values.items())
    if ready:text+='INIT_ABSENT\n'
    else:text+=frame('INIT_MEMBERS',str(pid)+'\n')+frame('INIT_TYPE','domain\n')+frame('INIT_ENABLED','')
    return text

def result(number):
    return dict(schema=1,event='result') | {
        'pass':number,'pid':702,'status':'Completed','shared_effects_possible':True,
        'network':dict(status='Completed',assessment='Observed',runtime='External',family='Present(NfTables)',ipv6=[],
          modules=[dict(name=name,outcome='Success',spawned=True,joined=True,reaped=True,ownership_lost=False,exit=0,
                        after=dict(loaded='Present(false)',builtin_index='Absent',available_index='Present(true)')) for name in MODULES]),
        'container':dict(status='Completed',layout='V2',mount=dict(attempted=True,result='Ok(())'),
          init=dict(attempted=number==1,result='Ok(())'),migration=dict(attempted=True,result='Ok(())'),
          available=CONTROLLERS,enabled_before=[] if number==1 else CONTROLLERS,
          enabled_after={'names':CONTROLLERS},missing_after=[],shared_effects_possible=True,
          attempts=[dict(controllers=CONTROLLERS,mutation=dict(attempted=True,result='Ok(())'))])}

def results():
    values=[result(1),result(2)]
    return values

def consumer(scenario, rows=None):
    pid=701 if scenario=='signal' else 702
    event=lambda name:dict(schema=1,event=name,pid=pid)
    events=[event('READY')]
    if scenario=='signal':events.append(dict(schema=1,event='protocol_failure',phase='READY',reason='cancelled',preparation_started=False,shared_effects_possible=False))
    else:
        rows=results() if rows is None else rows
        events += [rows[0],event('FIRST'),rows[1],event('SECOND'),event('DONE')]
    out=f'LAUNCH_PID {pid}\n'+frame('PRIVATE_BEFORE_BIND',mounts())+''.join(json.dumps(v)+'\n' for v in events)
    return ('EXIT '+('1' if scenario=='signal' else '0')+'\n'+frame('STDOUT',out)+frame('STDERR',''))

def semantic_log(rows=None):
    pin=support.load(support.HERE/'inputs.json')['namespace_launcher']
    text=frame('SETUP',pin['version']+'\nUNSHARE_SHA256 '+pin['sha256']+'  '+pin['program']+'\n')
    for name in ['help','version','print']:
        out=config_text() if name=='print' else ''
        err=(support.read(support.ROOT/'crates/rubix-kube/src/help.txt').decode() if name=='help' else
             json.dumps(dict(level='info',message='kubesolo version',version='0.1.0'))+'\n' if name=='version' else '')
        text+=frame('CLI_'+name,'EXIT 0\n'+frame('STDOUT',out)+frame('STDERR',err))
    text+=frame('INJECTED_TESTS',test_log())
    for mode,reason in [('eof','eof'),('wrong','wrong_byte')]:
        events=[dict(schema=1,event='READY',pid=700),dict(schema=1,event='protocol_failure',phase='READY',reason=reason,preparation_started=False,shared_effects_possible=False)]
        text+=frame('PROTOCOL_'+mode,'EXIT 1\n'+''.join(json.dumps(v)+'\n' for v in events)+frame('STDERR',''))
    text+=frame('OBS_signal_READY',observation('signal_READY'))+frame('CONSUMER_signal',consumer('signal'))
    text+='SIGNAL_NO_PREPARATION\nPID_ABSENT 701\n'
    for phase in ['real_READY','real_FIRST','real_SECOND']:text+=frame('OBS_'+phase,observation(phase))
    text+=frame('CONSUMER_real',consumer('real',rows))+'REAL_PROCESS_EXITED\nPID_ABSENT 702\nCONTAINER_GUEST_COMPLETE\n'
    return text

def build_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True);meta=metadata()
    for name in BINARIES:(directory/name).write_bytes(name.encode())
    nonce='d'*32
    build=''.join('#19 0.1 '+meta['files'][name]['sha256']+'  /out/'+name+'\n' for name in BINARIES)
    build='#19 0.1 RUBIX_BUILD_BIND_BEGIN '+nonce+'\n'+build+'#19 0.1 RUBIX_BUILD_BIND_END '+nonce+'\n'
    run=''.join(meta['files'][name]['sha256']+'  /out/'+name+'\n' for name in BINARIES)+test_log()
    for name in ['version','help','print-config']:
        body=(json.dumps(dict(level='info',message='kubesolo version',version='0.1.0'))+'\n' if name=='version' else
              support.read(support.ROOT/'crates/rubix-kube/src/help.txt').decode() if name=='help' else config_text())
        run+=frame('CLI_'+name,body)
    policy_names=['tests::cancelled_result_publishes_once_and_is_terminal',
                  'tests::uncertain_cleanup_is_quiet_terminal_even_when_observer_is_settled']
    policy=''.join('test '+name+' ... ok\n' for name in policy_names)
    policy+='test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
    build=frame('POLICY_TESTS',policy)+build
    (directory/'build.log').write_text(build);(directory/'run.log').write_text(run)
    write_json(directory/'source-hashes.json',support.inventory());write_json(directory/'artifact.json',meta)
    tag='rubix-node-container-'+'b'*32
    report=dict(schema=1,revision=meta['revision'],dirty=False,tag=tag,containers=[tag+'-test'],command=verify_build.command(tag),
                errors=[],cleanup_errors=[],remaining_containers=[],remaining_images=[],image_id='sha256:'+'c'*64,
                helper_sha256=support.digest(support.ROOT/'tools/defaults/capture.py'),source_sha256=support.digest(directory/'source-hashes.json'),
                build_sha256=support.digest(directory/'build.log'),run_sha256=support.digest(directory/'run.log'),
                build_nonce=nonce,build_command=verify_build.build_command(tag,nonce,'/tmp/rubix-container-build-synthetic'))
    write_json(directory/'receipt.json',report)
    return report

class StrictReaders(unittest.TestCase):
    def test_json_duplicate_nonfinite_and_float_rejection(self):
        for raw in ['{"x":1,"x":2}','{"x":NaN}','{"x":1.5}']:
            with self.subTest(raw=raw),self.assertRaises(ValueError):support.loads(raw)
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'cloud'
            for raw in ['{"x":1e999}','{"x":NaN}','{"x":1,"x":2}']:
                p.write_text(raw)
                with self.assertRaises(ValueError):verify.vm_load(p)
    def test_bounded_regular_nofollow_reads(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'raw';p.write_bytes(b'12345')
            with self.assertRaises(ValueError):support.read(p,4)
            p.unlink();p.symlink_to('missing')
            with self.assertRaises(OSError):support.read(p)
    def test_publication_metadata_excluded_actual_source_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/node-container/verify.py']:
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('x')
            with patch.object(support,'ROOT',root):
                before=support.inventory()
                (root/'tools/node-container/README.md').write_text('publication')
                (root/'tools/node-container/provenance.json').write_text('{}')
                self.assertEqual(support.inventory(),before)
                (root/'tools/node-container/verify.py').write_text('changed')
                self.assertNotEqual(support.inventory(),before)

class SemanticMutations(unittest.TestCase):
    def test_complete_invented_log(self):
        self.assertEqual(verify.semantic(semantic_log(),metadata()),dict(passes=2,controllers=CONTROLLERS,family='Present(NfTables)',injected_tests=38))
    def test_mount_bounds_cycles_sharing_and_complete_records(self):
        for raw in [mounts()[:-1],mounts()+mounts(),mounts().replace('2 1','2 2'),mounts().replace('2 1','2 99'),mounts().replace('/proc','relative',1),'x'*1048577]:
            with self.subTest(raw=raw[:40]),self.assertRaises(ValueError):verify.mount_table(raw)
        for phase in ['real_READY','real_FIRST']:
            body=observation(phase);body=body.replace(frame('CANDIDATE_MOUNTS',mounts(phase!='real_READY')),frame('CANDIDATE_MOUNTS',mounts(phase=='real_READY')))
            with self.assertRaises(ValueError):verify.observation(body,phase,metadata()['files']['prepare_node_host']['sha256'])
    def test_pid_executable_namespace_domain_and_membership(self):
        body=observation('real_READY');digest=metadata()['files']['prepare_node_host']['sha256']
        changes=[('PID 702','PID 999'),(digest,'0'*64),('mnt:[201]','mnt:[101]'),('cgroup:[202]','cgroup:[102]'),
                 ('VISIBLE_ROOT_ID 4:2','VISIBLE_ROOT_ID 4:1'),('domain\n','threaded\n'),('ROOT_MEMBERS_BEGIN\n702','ROOT_MEMBERS_BEGIN\n701'),
                 ('ROOT_ENABLED_BEGIN\n','ROOT_ENABLED_BEGIN\ncpu\n'),(') S ',') Z '),('12345','0')]
        for old,new in changes:
            with self.subTest(old=old),self.assertRaises(ValueError):verify.observation(body.replace(old,new,1),'real_READY',digest)
    def test_no_partial_or_false_delegation(self):
        body=observation('real_FIRST');digest=metadata()['files']['prepare_node_host']['sha256']
        for old,new in [('INIT_MEMBERS_BEGIN\n702','INIT_MEMBERS_BEGIN\n701'),('ROOT_MEMBERS_BEGIN\n','ROOT_MEMBERS_BEGIN\n702\n'),
                        ('INIT_ENABLED_BEGIN\n','INIT_ENABLED_BEGIN\ncpu\n'),('ROOT_ENABLED_BEGIN\ncpu cpuset io memory pids','ROOT_ENABLED_BEGIN\ncpu'),
                        ('AVAILABLE_BEGIN\ncpu cpuset io memory pids','AVAILABLE_BEGIN\ncpu'),('0::/rubix-container-real/init','0::/rubix-container-real')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify.observation(body.replace(old,new,1),'real_FIRST',digest)
    def test_same_live_identity_and_outside_scope_across_barriers(self):
        text=semantic_log()
        for old,new in [('12345','12346'),('MNT_NS mnt:[201]','MNT_NS mnt:[301]'),('ROOT_ID 4:2','ROOT_ID 4:9'),
                        ('a'*64,'b'*64),('PARENT_ENABLED_BEGIN\ncpu cpuset io memory pids','PARENT_ENABLED_BEGIN\ncpu cpuset io memory pids hugetlb')]:
            altered=observation('real_SECOND').replace(old,new)
            changed=text.replace(frame('OBS_real_SECOND',observation('real_SECOND')),frame('OBS_real_SECOND',altered))
            with self.subTest(old=old),self.assertRaises(ValueError):verify.semantic(changed,metadata())
    def test_protocol_cancellation_and_no_early_or_duplicate_result(self):
        text=semantic_log()
        for old,new in [('"preparation_started": false','"preparation_started": true'),('"shared_effects_possible": false','"shared_effects_possible": true'),
                        ('"reason": "cancelled"','"reason": "timeout"'),('"event": "FIRST"','"event": "SECOND"'),
                        ('PID_ABSENT 702','PID_ABSENT 999'),('CONTAINER_GUEST_COMPLETE\n','')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify.semantic(text.replace(old,new,1),metadata())
    def test_reports_cannot_claim_success_without_independent_observations(self):
        changes=[lambda r:r['container']['enabled_after'].update(names=['cpu']),lambda r:r['container']['migration'].update(result='Err(Io)'),
                 lambda r:r['container']['init'].update(attempted=False),lambda r:r['container'].update(missing_after=['memory']),
                 lambda r:r['network']['modules'].reverse(),lambda r:r['network']['modules'][0].update(joined=False),
                 lambda r:r['network']['modules'][0].update(outcome='Failed',exit=1),lambda r:r.update(status='Cancelled')]
        for change in changes:
            rows=results();change(rows[0])
            with self.assertRaises(ValueError):verify.semantic(semantic_log(rows),metadata())
    def test_configuration_parent_duplicate_and_exact_guest_flags(self):
        for raw in [config_text().replace('containerMode: true','containerMode: false'),config_text().replace('network:','wrong:'),
                    config_text()+'runtime:\n',config_text().replace('  disableIPv6','    disableIPv6'),config_text().replace('endpoint: unix','endpoint: tcp')]:
            with self.assertRaises(ValueError):verify_build.verify_config(raw)
    def test_exact_injected_test_inventory_rejects_omission_duplicate_failure(self):
        raw=test_log();name=next(iter(support.load(support.HERE/'test-inventory.json').values()))[0]
        for text in [raw.replace('test '+name+' ... ok\n',''),raw.replace('test '+name+' ... ok\n',('test '+name+' ... ok\n')*2),raw.replace('... ok','... FAILED',1),raw.replace('0 ignored;','1 ignored;',1)]:
            with self.assertRaises(ValueError):verify_build.verify_tests(text)


    def test_protocol_ready_requires_integer_schema_one(self):
        text=semantic_log()
        for value in [True,2]:
            old=json.dumps(dict(schema=1,event='READY',pid=700))
            new=json.dumps(dict(schema=value,event='READY',pid=700))
            with self.subTest(value=value),self.assertRaises(ValueError):verify.semantic(text.replace(old,new,1),metadata())
    def test_pid_observations_reject_out_of_range_kernel_pid(self):
        with self.assertRaises(ValueError):verify.pids('4294967296\n')
    def test_injected_log_rejects_unexpected_failed_case(self):
        raw=test_log().replace('TEST_rubix_kube_END\n','test surprise ... FAILED\nTEST_rubix_kube_END\n')
        with self.assertRaises(ValueError):verify_build.verify_tests(raw)

class BuildMutations(unittest.TestCase):
    def test_complete_build_and_four_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);build_fixture(p);self.assertEqual(verify_build.verify(p),metadata())
    def test_builder_nonce_frames_cannot_be_missing_duplicated_or_relabelled(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report=build_fixture(p);raw=(p/'build.log').read_bytes();nonce=report['build_nonce']
            self.assertEqual(set(verify_build.builder_hashes(raw,nonce)),set(BINARIES))
            for altered in [b'',raw+raw,raw.replace(nonce.encode(),b'e'*32),raw.replace(b'/out/host_network',b'/out/unknown'),raw.replace(b'RUBIX_BUILD_BIND_END',b'RUBIX_BUILD_BIND_BROKEN')]:
                with self.assertRaises(ValueError):verify_build.builder_hashes(altered,nonce)
    def test_builder_policy_proof_rejects_extra_failed_test(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report=build_fixture(p)
            raw=(p/'build.log').read_bytes().replace(b'POLICY_TESTS_END\n',b'test surprise ... FAILED\nPOLICY_TESTS_END\n')
            with self.assertRaises(ValueError):verify_build.builder_hashes(raw,report['build_nonce'])
    def test_raw_builder_change_fails_even_with_refreshed_log_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report=build_fixture(p);raw=(p/'build.log').read_text();old=metadata()['files']['rubix_kube']['sha256']
            (p/'build.log').write_text(raw.replace(old,'e'*64));report['build_sha256']=support.digest(p/'build.log');write_json(p/'receipt.json',report)
            with self.assertRaises(ValueError):verify_build.verify(p)
    def test_build_source_cleanup_and_artifact_bindings(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report=build_fixture(p)
            for key,value in [('dirty',True),('revision','notcommit'),('helper_sha256','0'*64),('source_sha256','0'*64),('run_sha256','0'*64),
                              ('build_sha256','0'*64),('cleanup_errors',['failure']),('remaining_containers',['leak']),('remaining_images',['leak']),('command',[])]:
                changed=copy.deepcopy(report);changed[key]=value;write_json(p/'receipt.json',changed)
                with self.subTest(key=key),self.assertRaises(ValueError):verify_build.verify(p)
            build_fixture(p);source=support.load(p/'source-hashes.json');source['Cargo.lock']='0'*64;write_json(p/'source-hashes.json',source)
            report['source_sha256']=support.digest(p/'source-hashes.json');write_json(p/'receipt.json',report)
            with self.assertRaises(ValueError):verify_build.verify(p)
            for name in BINARIES:
                build_fixture(p);(p/name).write_bytes(b'changed')
                with self.subTest(name=name),self.assertRaises(ValueError):verify_build.verify(p)
    def test_equal_runtime_and_metadata_substitution_cannot_replace_builder(self):
        for name in BINARIES:
            with tempfile.TemporaryDirectory() as tmp:
                p=Path(tmp);report=build_fixture(p);meta=metadata();old=meta['files'][name]['sha256'];replacement=b'substituted'
                (p/name).write_bytes(replacement);meta['files'][name]=dict(sha256=sha(replacement),size=len(replacement));write_json(p/'artifact.json',meta)
                raw=(p/'run.log').read_text().replace(old,sha(replacement));(p/'run.log').write_text(raw)
                report['run_sha256']=support.digest(p/'run.log');write_json(p/'receipt.json',report)
                with self.subTest(name=name),self.assertRaises(ValueError):verify_build.verify(p)

def guest_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True)
    build_fixture(directory/'artifact-build')
    metadata=support.load(directory/'artifact-build/artifact.json')
    inputs=support.load(support.HERE/'inputs.json')
    cloud=dict(status='done',errors=[],recoverable_errors={})
    cloud.update({stage:dict(errors=[],start=0.1,finished=0.2) for stage in ['init-local','init','modules-config','modules-final']})
    write_json(directory/'cloud-init.stdout',cloud)
    text=semantic_log();(directory/'container-cases.stdout').write_text(text)
    names={'launch.sh','namespace.sh',*BINARIES,'repo/aarch64/APKINDEX.tar.gz'}
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
    sources=['capture.py','inputs.json','guest.sh','verify.py','support.py','launch.sh','namespace.sh']
    report=dict(status='passed',errors=[],owned_process_group_absent=True,
                owned_temporary_directory_removed=True,explicit_privilege_opt_in=True,
                qemu_exit_code=0,shutdown='guest-poweroff',privileged_mount_probe='passed',
                revision='b'*40,working_tree_snapshot=False,adapter='qemu-disposable-node-container',
                source_sha256={name:support.digest(support.HERE/name) for name in sources},
                baseline_helper_sha256=support.digest(support.ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),
                inherited_vm_sha256=support.digest(support.ROOT/'tools/parity/vm/run.py'),
                build_verifier_sha256=support.digest(support.HERE/'verify_build.py'),
                build_support_sha256=support.digest(support.HERE/'support.py'),
                inputs=inputs,tools={name:dict(sha256=value) for name,value in inputs['host_tools'].items()},
                firmware=inputs['firmware'],artifact_sha256=metadata['files']['prepare_node_host']['sha256'],cloud_init=cloud,cloud_init_exit=0,
                serial_log_truncated=False,owned_temporary_directory=private,ssh_forward=port,
                qemu_argv=command,guest_host_public_key='ssh-ed25519 YWJjZA== rubix-alpine-fixture-host',
                observation=text)
    write_json(directory/'result.json',report)
    return report


class GuestMutations(unittest.TestCase):
    def test_complete_guest_and_receipt_mutations(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);original=guest_fixture(p)
            self.assertEqual(verify.verify_guest(p)['passes'],2)
            for key,value in [('working_tree_snapshot',True),('errors',['failed']),('owned_process_group_absent',False),
                              ('owned_temporary_directory_removed',False),('qemu_exit_code',True),('qemu_exit_code',1),
                              ('shutdown','forced-process-termination'),('source_sha256',{}),('build_verifier_sha256','0'*64),
                              ('artifact_sha256','0'*64),('qemu_argv',[]),('ssh_forward','0.0.0.0:22222'),
                              ('serial_log_truncated',True),('observation','forged')]:
                changed=copy.deepcopy(original);changed[key]=value;write_json(p/'result.json',changed)
                with self.subTest(key=key),self.assertRaises(ValueError):verify.verify_guest(p)
    def test_guest_requires_every_input_hash_and_cloud_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report=guest_fixture(p)
            (p/'verify-guest-inputs.stdout').write_text('prepare_node_host: OK\n')
            with self.assertRaises(ValueError):verify.verify_guest(p)
            guest_fixture(p);(p/'verify-guest-inputs.stderr').write_text('hash failure\n')
            with self.assertRaises(ValueError):verify.verify_guest(p)
            guest_fixture(p);cloud=copy.deepcopy(report['cloud_init']);cloud['init']['errors']=['failed'];write_json(p/'cloud-init.stdout',cloud)
            with self.assertRaises(ValueError):verify.verify_guest(p)
            report['cloud_init']=cloud;write_json(p/'result.json',report)
            with self.assertRaises(ValueError):verify.verify_guest(p)
    def test_failed_cleanup_preserves_failure_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);report={'containers':['owned'],'errors':['test failed'],'cleanup_errors':[]}
            with patch.object(build.helper.subprocess,'run',side_effect=OSError('daemon unavailable')):
                build.helper.finish(report,p,'owned')
            saved=support.load(p/'receipt.json')
            self.assertEqual(saved['errors'],['test failed'])
            self.assertTrue(saved['cleanup_errors'])
            self.assertIsNone(saved['remaining_containers'])
            self.assertIsNone(saved['remaining_images'])
    def test_raw_inventory_exact_hash_and_private_key_rejection(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);e=p/'evidence';e.mkdir();f=e/'extra';f.write_bytes(b'raw')
            write_json(p/'provenance.json',{'files':{}})
            with self.assertRaises(ValueError):verify.verify(p)
            write_json(p/'provenance.json',{'files':{'extra':'0'*64}})
            with self.assertRaises(ValueError):verify.verify(p)
            f.write_bytes(b'BEGIN OPENSSH PRIVATE KEY');write_json(p/'provenance.json',{'files':{'extra':sha(f.read_bytes())}})
            with self.assertRaisesRegex(ValueError,'private'):verify.verify(p)

class PublishedEvidence(unittest.TestCase):
    # Mandatory gates deliberately fail until real evidence is published; no skips.
    def test_first_frozen_build(self):
        verify_build.verify(support.HERE/'evidence/first/artifact-build',binary=False)
    def test_repeat_frozen_build(self):
        verify_build.verify(support.HERE/'evidence/repeat/artifact-build',binary=False)
    def test_first_live_guest(self):
        verify.verify_guest(support.HERE/'evidence/first')
    def test_repeat_live_guest(self):
        verify.verify_guest(support.HERE/'evidence/repeat')
    def test_exact_published_inventory(self):
        verify.verify(support.HERE)

if __name__=='__main__':unittest.main()
