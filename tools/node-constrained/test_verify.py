"""Independent invented-record mutation tests; synthetic logs are never VM evidence."""
import copy
import hashlib
import json
from pathlib import Path
import stat
import os
import resource
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import support
import verify
import verify_build

SHA = 'a'*64
META = {'files': {'prepare_node_host': {'sha256': SHA}}, 'revision': 'b'*40}
MODULES = ['br_netfilter','overlay','xt_comment','xt_conntrack','xt_MASQUERADE','xt_addrtype','xt_multiport','xt_nat','nft_compat','nft_numgen','nft_redir','nft_limit','nft_tproxy']
CONTROLS = ['/proc/sys/net/ipv6/conf/'+name+'/disable_ipv6' for name in ['all','default','lo']]

def write(path, value):
    path.write_text(json.dumps(value)+'\n')

def mounts(readonly=True):
    mode='ro' if readonly else 'rw'
    return ('1 0 8:1 / / rw - ext4 /dev/root rw\n'
            '2 1 0:2 / /proc rw - proc proc rw\n'
            f'3 2 0:2 /sys /proc/sys {mode},nosuid,nodev,noexec - proc proc rw\n')

def identity(pid, namespace, digest=SHA):
    return dict(pid=pid,starttime=1000+pid,exe_sha256=digest,mnt=f'mnt:[{namespace}]',cgroup='0::/fixture\n')

def keeper():
    value=identity(100,10,'c'*64)
    value['socket']=dict(device=1,inode=20,mode=stat.S_IFSOCK|0o600,uid=0,gid=0)
    value['configuration']=dict(identity=dict(device=1,inode=21,mode=stat.S_IFREG|0o600,uid=0,gid=0),sha256='d'*64)
    return value

def config():
    return ('apiVersion: kubesolo.io/v1alpha1\nkind: Config\nnetwork:\n  disableIPv6: true\n'
            'runtime:\n  containerMode: false\n  endpoint: unix:///tmp/external-runtime/containerd.sock\n')

def report(case, number, pid):
    stopped=case in ['guard','cancel']
    status={'guard':'GuardStopped','cancel':'Cancelled'}.get(case,'Completed')
    modules=[]
    for name in ([] if case=='guard' else MODULES[:1] if case=='cancel' else MODULES):
        modules.append(dict(name=name,outcome='Cancelled' if case=='cancel' else 'Success',spawned=True,joined=True,reaped=True,
                            ownership_lost=False,exit=None if case=='cancel' else 0,
                            after=None if case=='cancel' else dict(loaded='Present(true)',builtin_index='Absent',available_index='Present(true)')))
    container=dict(status='NotStarted' if stopped else 'NotRequested',layout=None,mount=None,init=None,migration=None,available=[],enabled_before=[],enabled_after=None,missing_after=[],attempts=[],shared_effects_possible=False)
    network=dict(status=status,assessment='Unknown' if case=='guard' else 'Observed',runtime='External',family='Unknown(Io)' if case=='guard' else 'Present(NfTables)',modules=modules,
                 ipv6=[] if stopped else [dict(path=p,outcome='AlreadyDisabled' if case=='correct' else 'WriteFailed(Io)') for p in CONTROLS])
    return dict(schema=1,event='result',pass_=number,pid=pid,status=status,shared_effects_possible=case!='guard',network=network,container=container)

def result(case,number,pid):
    value=report(case,number,pid);value['pass']=value.pop('pass_');return value

def records():
    pin=support.load(support.HERE/'inputs.json')['namespace_launcher']
    rows=[dict(schema=1,event='setup',launcher_argv=['/usr/bin/unshare','--mount','--propagation','private'],unshare_version=pin['version'],unshare_sha256=pin['sha256'],candidate_sha256=SHA,external=keeper()),
          dict(schema=1,event='cli',argv=['--no-container-mode','--disable-ipv6','--container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock','--print-config'],exit=0,stdout=config(),stderr='')]
    for index,case in enumerate(['correct','needs_write','guard','cancel']):
        pid=200+index;value=1 if case=='correct' else 0
        before=dict(schema=1,event='before',case=case,external=keeper(),sentinels={'/etc/init.d':'init','/etc/cni/net.d':'cni','/tmp/external-runtime':'runtime'},observer_mnt='mnt:[10]',root_enabled='cpu memory\n',observer_mounts=mounts(False),outside_values=[value]*3)
        rows.append(before)
        for phase in (['READY'] if case in ['guard','cancel'] else ['READY','FIRST','SECOND']):
            row=copy.deepcopy(before);row.update(event='observation',phase=phase,identity=identity(pid,20+index),mounts=mounts(),visible_values=[value]*3);rows.append(row)
        if case=='cancel':rows.append(dict(schema=1,event='double_started',case=case,module='br_netfilter',identity=identity(300,20+index,'e'*64)))
        events=[dict(schema=1,event='READY',pid=pid),result(case,1,pid)]
        if case not in ['guard','cancel']:events += [dict(schema=1,event='FIRST',pid=pid),result(case,2,pid),dict(schema=1,event='SECOND',pid=pid),dict(schema=1,event='DONE',pid=pid)]
        rows.append(dict(schema=1,event='consumer',case=case,pid=pid,exit=1 if case in ['guard','cancel'] else 0,events=events,stderr=''))
        if case=='guard':rows.append(dict(schema=1,event='guard_calls',value='GUARD_DOUBLE\n'))
        if case=='cancel':rows.append(dict(schema=1,event='double_absent',pid=300))
        after=copy.deepcopy(before);after.update(event='after',pid_absent=pid);rows.append(after)
    rows.append(dict(schema=1,event='complete',keeper_pid_absent=100,socket_removed=True))
    return rows

def encoded(rows):
    return ''.join(json.dumps(row)+'\n' for row in rows)

def event(rows,name,case=None,phase=None):
    return next(r for r in rows if r['event']==name and (case is None or r.get('case')==case) and (phase is None or r.get('phase')==phase))

class StrictReaders(unittest.TestCase):
    def test_json_duplicate_nonfinite_and_float(self):
        for raw in ['{"x":1,"x":2}','{"x":NaN}','{"x":Infinity}','{"x":1.0}']:
            with self.assertRaises(ValueError):support.loads(raw)
    def test_regular_bounded_nofollow_reads(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'data';p.write_bytes(b'12345')
            with self.assertRaises(ValueError):support.read(p,4)
            link=Path(tmp)/'link';link.symlink_to(p)
            with self.assertRaises(OSError):support.read(link)
            with self.assertRaises((ValueError,OSError)):support.read(Path(tmp))
    def test_fixture_inventory_excludes_publication_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);here=root/'tools/node-constrained';here.mkdir(parents=True)
            source=here/'guest.py';source.write_text('original')
            with patch.object(support,'ROOT',root),patch.object(support,'HERE',here):
                original=support.inventory();(here/'README.md').write_text('facts');(here/'provenance.json').write_text('{}');(here/'evidence').mkdir();(here/'evidence/raw').write_text('record')
                self.assertEqual(support.inventory(),original)
                source.write_text('changed');self.assertNotEqual(support.inventory(),original)

class SemanticMutations(unittest.TestCase):
    def reject(self, change):
        rows=records();change(rows)
        with self.assertRaises(ValueError):verify.semantic(encoded(rows),META)
    def test_complete_invented_records(self):
        self.assertEqual(verify.semantic(encoded(records()),META),dict(cases=['correct','needs_write','guard','cancel'],real_passes=4,read_only=True,external_preserved=True,nft_only_kernel_qualified=False))
    def test_exact_order_missing_extra_and_duplicate_events(self):
        for change in [lambda r:r.pop(),lambda r:r.append(r[-1]),lambda r:r.insert(2,copy.deepcopy(r[2])),lambda r:r.reverse()]:self.reject(change)
    def test_kernel_pid_and_schema_strict_types(self):
        for value in [0,True,2**32]:self.reject(lambda r,v=value:event(r,'observation','correct','READY')['identity'].update(pid=v))
        for value in [True,2]:self.reject(lambda r,v=value:r[0].update(schema=v))
    def test_config_exact_flags_and_semantic_parent(self):
        for change in [lambda r:r[1]['argv'].append('--container-mode'),lambda r:r[1].update(stdout=config().replace('runtime:','wrong:')),lambda r:r[1].update(stdout=config().replace('containerMode: false','containerMode: true')),lambda r:r[1].update(stderr='warning')]:self.reject(change)
    def test_same_live_pid_starttime_exe_namespace(self):
        for key,value in [('pid',999),('starttime',999),('exe_sha256','f'*64),('mnt','mnt:[99]')]:self.reject(lambda r,k=key,v=value:event(r,'observation','correct','SECOND')['identity'].update({k:v}))
        self.reject(lambda r:event(r,'observation','correct','READY')['identity'].update(mnt='mnt:[10]'))
    def test_live_external_socket_configuration_and_identity_preserved(self):
        for change in [lambda e:e.update(pid=999),lambda e:e.update(starttime=999),lambda e:e.update(exe_sha256='f'*64),lambda e:e['socket'].update(inode=99),lambda e:e['configuration'].update(sha256='f'*64)]:
            self.reject(lambda r,c=change:c(event(r,'observation','needs_write','SECOND')['external']))
        self.reject(lambda r:r[0]['external']['socket'].update(mode=stat.S_IFREG|0o600))
    def test_readonly_mount_and_descendants_cannot_be_writable(self):
        self.reject(lambda r:event(r,'observation','correct','READY').update(mounts=mounts(False)))
        self.reject(lambda r:event(r,'observation','correct','READY').update(mounts=mounts()+'4 3 0:2 /sys/net /proc/sys/net rw - proc proc rw\n'))
        self.reject(lambda r:event(r,'observation','correct','READY').update(mounts=mounts().replace(' rw - ext4',' rw shared:1 - ext4')))
    def test_mount_table_complete_acyclic_and_bounded(self):
        for text in [mounts()[:-1],mounts()+mounts(),mounts().replace('3 2','3 3'),mounts().replace('3 2','3 99'),'x'*1048577]:
            with self.assertRaises(ValueError):verify.mount_table(text)
    def test_independent_readbacks_and_outside_sentinels(self):
        for key,value in [('visible_values',[1,1,1]),('outside_values',[1,1,1]),('observer_mnt','mnt:[999]'),('observer_mounts',mounts()),('sentinels',{})]:self.reject(lambda r,k=key,v=value:event(r,'observation','needs_write','FIRST').update({k:v}))
    def test_readonly_failures_never_report_success_or_shortcircuit_attempts(self):
        for change in [lambda n:n['ipv6'][0].update(outcome='ObservedDisabled'),lambda n:n['ipv6'].pop(),lambda n:n['modules'].reverse(),lambda n:n['modules'][0].update(joined=False),lambda n:n['modules'][0].update(ownership_lost=True)]:self.reject(lambda r,c=change:c(event(r,'consumer','needs_write')['events'][1]['network']))
    def test_no_container_mode_effects(self):
        for key,value in [('status','Completed'),('shared_effects_possible',True),('mount',{}),('attempts',[{}]),('migration',{})]:self.reject(lambda r,k=key,v=value:event(r,'consumer','correct')['events'][1]['container'].update({k:v}))
    def test_guard_and_cancellation_stop_later_effects(self):
        self.reject(lambda r:event(r,'consumer','guard')['events'][1]['network']['modules'].append({'name':'overlay'}))
        self.reject(lambda r:event(r,'consumer','cancel')['events'][1]['network']['ipv6'].append({'path':CONTROLS[0],'outcome':'AlreadyDisabled'}))
        self.reject(lambda r:event(r,'consumer','cancel')['events'].append({'event':'DONE'}))
        self.reject(lambda r:event(r,'double_absent').update(pid=999))
    def test_cgroup_controller_state_and_launcher_scope_unchanged(self):
        self.reject(lambda r:event(r,'observation','correct','SECOND')['identity'].update(cgroup='0::/migrated\n'))
        self.reject(lambda r:event(r,'observation','correct','SECOND').update(root_enabled='cpu memory io\n'))
        self.reject(lambda r:r[0]['launcher_argv'].insert(2,'--cgroup'))
        self.reject(lambda r:r[0]['launcher_argv'].insert(2,'--fork'))
    def test_boolean_scalar_readbacks_are_not_numeric_kernel_values(self):
        def mutate(rows):
            for row in rows:
                if row.get('case')=='correct' and 'outside_values' in row:
                    row['outside_values']=[True]*3
        self.reject(mutate)

    def test_reaping_and_keeper_cleanup_required(self):
        self.reject(lambda r:event(r,'after','correct').update(pid_absent=999))
        self.reject(lambda r:r[-1].update(socket_removed=False))
        self.reject(lambda r:r[-1].update(keeper_pid_absent=999))

def build_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True)
    approved=verify_build.approved
    meta=dict(target='aarch64-unknown-linux-musl',revision='b'*40,
              files={name:dict(sha256=hashlib.sha256(name.encode()).hexdigest(),size=len(name)) for name in verify_build.BINARIES})
    # Retain the shared invented candidate digest used by independent JSONL templates.
    meta['files']['prepare_node_host']['sha256']=SHA
    nonce='d'*32;tag='rubix-node-container-'+'e'*32
    def frame(name,body):return name+'_BEGIN\n'+body+name+'_END\n'
    names=['tests::cancelled_result_publishes_once_and_is_terminal','tests::uncertain_cleanup_is_quiet_terminal_even_when_observer_is_settled']
    policy=''.join('test '+name+' ... ok\n' for name in names)+'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
    build=frame('POLICY_TESTS',policy)+'#19 0.1 RUBIX_BUILD_BIND_BEGIN '+nonce+'\n'
    build+=''.join('#19 0.1 '+value['sha256']+'  /out/'+name+'\n' for name,value in meta['files'].items())
    build+='#19 0.1 RUBIX_BUILD_BIND_END '+nonce+'\n'
    run=''.join(value['sha256']+'  /out/'+name+'\n' for name,value in meta['files'].items())
    tests=support.load(support.ROOT/'tools/node-container/test-inventory.json')
    for name,items in tests.items():
        run+=frame('TEST_'+name,''.join('test '+item+' ... ok\n' for item in items)+f'test result: ok. {len(items)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n')
    parent_config=config().replace('disableIPv6: true','disableIPv6: false').replace('containerMode: false','containerMode: true')
    for name,body in [('version',json.dumps(dict(level='info',message='kubesolo version',version='0.1.0'))+'\n'),('help',support.read(support.ROOT/'crates/rubix-kube/src/help.txt').decode()),('print-config',parent_config)]:run+=frame('CLI_'+name,body)
    (directory/'build.log').write_text(build);(directory/'run.log').write_text(run)
    write(directory/'source-hashes.json',approved.inventory());write(directory/'artifact.json',meta)
    receipt=dict(schema=1,revision=meta['revision'],dirty=False,tag=tag,containers=[tag+'-test'],command=approved.command(tag),errors=[],cleanup_errors=[],remaining_containers=[],remaining_images=[],image_id='sha256:'+'f'*64,
                 helper_sha256=support.digest(support.ROOT/'tools/defaults/capture.py'),source_sha256=support.digest(directory/'source-hashes.json'),build_sha256=support.digest(directory/'build.log'),run_sha256=support.digest(directory/'run.log'),build_nonce=nonce,build_command=approved.build_command(tag,nonce,'/tmp/rubix-container-build-invented'))
    write(directory/'receipt.json',receipt);return meta


def guest_fixture(directory):
    directory.mkdir(parents=True,exist_ok=True);meta=build_fixture(directory/'artifact-build');inputs=support.load(support.HERE/'inputs.json')
    cloud=dict(status='done',errors=[],recoverable_errors={})
    cloud.update({name:dict(errors=[],start=0.1,finished=0.2) for name in ['init-local','init','modules-config','modules-final']})
    write(directory/'cloud-init.stdout',cloud)
    text=encoded(records());(directory/'constrained-cases.stdout').write_text(text)
    names={'guest.py','namespace.sh','guard-failure.sh','module-wait.sh',*verify_build.BINARIES,'repo/aarch64/APKINDEX.tar.gz'}
    names.update('repo/aarch64/'+name for name in inputs['packages']['selected'])
    (directory/'verify-guest-inputs.stdout').write_text(''.join(name+': OK\n' for name in sorted(names)));(directory/'verify-guest-inputs.stderr').write_bytes(b'')
    private='/tmp/rubix-vm-invented';port='127.0.0.1:22333'
    argv=['qemu-system-aarch64','-machine','virt,accel=hvf','-cpu','host','-smp','2','-m','2048','-display','none','-serial','stdio','-monitor','none','-qmp','unix:'+private+'/qmp.sock,server=on,wait=off',
          '-drive','if=pflash,format=raw,readonly=on,file=/opt/homebrew/share/qemu/edk2-aarch64-code.fd','-drive','if=pflash,format=raw,file='+private+'/vars.fd','-drive','if=virtio,format=qcow2,file='+private+'/disk.qcow2',
          '-drive','if=virtio,format=raw,readonly=on,file='+private+'/seed.iso','-netdev','user,id=n0,restrict=on,hostfwd=tcp:'+port+'-:22','-device','virtio-net-pci,netdev=n0']
    report=dict(status='passed',errors=[],owned_process_group_absent=True,owned_temporary_directory_removed=True,explicit_privilege_opt_in=True,qemu_exit_code=0,shutdown='guest-poweroff',privileged_mount_probe='passed',revision='c'*40,working_tree_snapshot=False,adapter='qemu-disposable-node-constrained',
                source_sha256={name:support.digest(support.HERE/name) for name in verify.SOURCES},baseline_helper_sha256=support.digest(support.ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),inherited_vm_sha256=support.digest(support.ROOT/'tools/parity/vm/run.py'),build_verifier_sha256=support.digest(support.HERE/'verify_build.py'),build_support_sha256=support.digest(support.HERE/'support.py'),
                approved_build_sources={name:support.digest(support.ROOT/'tools/node-container'/name) for name in ['verify_build.py','support.py','Build.Dockerfile','build.py','test-inventory.json']},inputs=inputs,tools={name:dict(sha256=value) for name,value in inputs['host_tools'].items()},firmware=inputs['firmware'],artifact_sha256=meta['files']['prepare_node_host']['sha256'],cloud_init=cloud,cloud_init_exit=0,serial_log_truncated=False,owned_temporary_directory=private,ssh_forward=port,qemu_argv=argv,guest_host_public_key='ssh-ed25519 YWJjZA== rubix-alpine-fixture-host',observation=text)
    write(directory/'result.json',report);return report


class GuestMutations(unittest.TestCase):
    def test_complete_invented_guest_and_historical_build_revision(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);report=guest_fixture(directory)
            self.assertEqual(verify.verify_guest(directory)['real_passes'],4)
            self.assertNotEqual(report['revision'],support.load(directory/'artifact-build/artifact.json')['revision'])
    def test_raw_receipt_source_tool_and_cleanup_bindings(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);original=guest_fixture(directory)
            for key,value in [('owned_process_group_absent',False),('owned_temporary_directory_removed',False),('qemu_exit_code',True),('errors',['failure']),('source_sha256',{}),('approved_build_sources',{}),('artifact_sha256','0'*64),('tools',{}),('firmware',{}),('working_tree_snapshot',True),('serial_log_truncated',True),('observation','different')]:
                changed=copy.deepcopy(original);changed[key]=value;write(directory/'result.json',changed)
                with self.subTest(key=key),self.assertRaises(ValueError):verify.verify_guest(directory)
    def test_guest_exact_input_inventory_and_cloud_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);guest_fixture(directory)
            p=directory/'verify-guest-inputs.stdout';original=p.read_text();p.write_text(original.replace('guest.py: OK\n',''))
            with self.assertRaises(ValueError):verify.verify_guest(directory)
            p.write_text(original);write(directory/'cloud-init.stdout',dict(status='error'))
            with self.assertRaises(ValueError):verify.verify_guest(directory)
    def test_builder_hash_cannot_be_replaced_by_equal_runtime_and_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);guest_fixture(directory);build=directory/'artifact-build'
            meta=support.load(build/'artifact.json');old=meta['files']['prepare_node_host']['sha256'];meta['files']['prepare_node_host']['sha256']='0'*64;write(build/'artifact.json',meta)
            p=build/'run.log';p.write_text(p.read_text().replace(old,'0'*64));receipt=support.load(build/'receipt.json');receipt['run_sha256']=support.digest(p);write(build/'receipt.json',receipt)
            with self.assertRaisesRegex(ValueError,'builder/runtime'):verify.verify_guest(directory)
    def test_current_compiled_inventory_and_build_failure_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp);guest_fixture(directory);build=directory/'artifact-build';receipt=support.load(build/'receipt.json')
            for key,value in [('dirty',True),('cleanup_errors',['failed']),('remaining_images',['owned']),('revision','invalid'),('build_sha256','0'*64)]:
                changed=copy.deepcopy(receipt);changed[key]=value;write(build/'receipt.json',changed)
                with self.subTest(key=key),self.assertRaises(ValueError):verify.verify_guest(directory)
            write(build/'receipt.json',receipt);inventory=support.load(build/'source-hashes.json');inventory['Cargo.lock']='0'*64;write(build/'source-hashes.json',inventory);receipt['source_sha256']=support.digest(build/'source-hashes.json');write(build/'receipt.json',receipt)
            with self.assertRaisesRegex(ValueError,'current compiled'):verify.verify_guest(directory)

class FailureDiagnostics(unittest.TestCase):
    def test_failure_logs_are_bounded_and_missing_logs_are_explicit(self):
        guest=support.module('constrained_guest_diagnostics',support.HERE/'guest.py')
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'log';path.write_bytes(b'x'*65536+b'OMITTED_TAIL')
            value=guest.failure_log(path)
            self.assertEqual(value,dict(text='x'*65536,truncated=True,error=None))
            path.unlink();value=guest.failure_log(path)
            self.assertEqual(value,dict(text='',truncated=False,error='FileNotFoundError'))
    def test_settlement_precedes_failed_log_publication_without_success_conversion(self):
        guest=support.module('constrained_guest_failure',support.HERE/'guest.py')
        class Process:
            pid=123
        process=Process();original=RuntimeError('original protocol assertion')
        with tempfile.TemporaryDirectory() as tmp:
            out=Path(tmp)/'out';err=Path(tmp)/'err';out.write_text('partial READY\n');err.write_text('consumer assertion\n')
            for cleanup_fails in [False,True]:
                calls=[];captured=[]
                def settle(value):
                    self.assertIs(value,process);calls.append('settled')
                    if cleanup_fails:raise OSError('owned cleanup failed')
                def emit(event,**fields):
                    self.assertEqual(calls,['settled']);captured.append((event,fields))
                with patch.object(guest,'settle',side_effect=settle),patch.object(guest,'emit',side_effect=emit):
                    guest.report_failure('needs_write',process,out,err,original)
                self.assertEqual(len(captured),1);event_name,row=captured[0]
                self.assertEqual(event_name,'consumer_failure')
                self.assertEqual((row['kind'],row['message']),('RuntimeError',str(original)))
                self.assertEqual(row['stdout']['text'],'partial READY\n');self.assertEqual(row['stderr']['text'],'consumer assertion\n')
                self.assertEqual(row['cleanup_error'],'OSError: owned cleanup failed' if cleanup_fails else None)
                rows=records();rows.insert(-1,dict(schema=1,event=event_name,**row))
                with self.assertRaises(ValueError):verify.semantic(encoded(rows),META)

class NamespaceToolView(unittest.TestCase):
    def run_view(self,source,reference,view,tool,double):
        raw=(support.HERE/'namespace.sh').read_text()
        begin='# MAKE_TOOL_VIEW_BEGIN\n';end='# MAKE_TOOL_VIEW_END'
        self.assertEqual(raw.count(begin),1);self.assertEqual(raw.count(end),1)
        function=raw.split(begin)[1].split(end)[0]
        # Execute only the marked file-view helper, never namespace setup or any mount.
        script='set -eu\n'+function+'\nmake_tool_view "$@"\n'
        def limits():
            resource.setrlimit(resource.RLIMIT_FSIZE,(1024*1024,1024*1024))
            resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        return subprocess.run(['/bin/sh','-c',script,'view-test',str(source),str(reference),str(view),tool,str(double)],capture_output=True,text=True,timeout=10,preexec_fn=limits)
    def test_large_executable_and_relative_aliases_preserved_under_file_limit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);source=root/'usr/sbin';source.mkdir(parents=True);external=root/'usr/bin';external.mkdir()
            payload=b'x'*(2*1024*1024);(source/'large').write_bytes(payload);(external/'outside').write_bytes(payload)
            (source/'inside-alias').symlink_to('large');(source/'outside-alias').symlink_to('../bin/outside')
            (source/'iptables').symlink_to('large');reference=root/'original';reference.symlink_to(source,target_is_directory=True)
            double=root/'double';double.write_text('#!/bin/sh\nexit 73\n');view=root/'view'
            result=self.run_view(source,reference,view,'iptables',double)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(os.readlink(view/'large'),str(reference/'large'))
            self.assertEqual(os.readlink(view/'inside-alias'),str(reference/'large'))
            self.assertEqual(os.readlink(view/'outside-alias'),str((external/'outside').resolve()))
            self.assertEqual((view/'inside-alias').read_bytes(),payload)
            self.assertEqual((view/'outside-alias').read_bytes(),payload)
            self.assertFalse((view/'iptables').is_symlink());self.assertEqual((view/'iptables').read_bytes(),double.read_bytes())
            self.assertEqual((source/'large').read_bytes(),payload);self.assertTrue((source/'iptables').is_symlink())
    def test_view_rejects_unbounded_nonregular_or_unapproved_inputs(self):
        for scenario in ['too-many','dangling','directory','tool','double']:
            with self.subTest(scenario=scenario),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp);source=root/'sbin';source.mkdir();reference=root/'original';reference.symlink_to(source,target_is_directory=True)
                (source/'tool').write_bytes(b'original');double=root/'double';double.write_text('#!/bin/sh\nexit 1\n')
                if scenario=='too-many':
                    for index in range(256):(source/str(index)).touch()
                elif scenario=='dangling':(source/'broken').symlink_to('missing')
                elif scenario=='directory':(source/'nested').mkdir()
                elif scenario=='double':double.write_bytes(b'x'*4097)
                result=self.run_view(source,reference,root/'view','unapproved' if scenario=='tool' else 'modprobe',double)
                self.assertNotEqual(result.returncode,0)
                self.assertEqual((source/'tool').read_bytes(),b'original')

class PublishedEvidence(unittest.TestCase):
    def test_first_approved_build_required(self):verify_build.verify(support.HERE/'evidence/first/artifact-build',binary=False)
    def test_repeat_approved_build_required(self):verify_build.verify(support.HERE/'evidence/repeat/artifact-build',binary=False)
    def test_first_real_guest_required(self):verify.verify_guest(support.HERE/'evidence/first')
    def test_repeat_real_guest_required(self):verify.verify_guest(support.HERE/'evidence/repeat')
    def test_exact_published_inventory_required(self):verify.verify(support.HERE)

if __name__=='__main__':unittest.main()
