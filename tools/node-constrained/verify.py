"""Strict read-only verification of owned-VM host preparation and independent observations."""
import hashlib
import json
import math
from pathlib import Path
import re
import sys
from support import HERE, ROOT, digest, load, loads, module, pairs, read, reject, require
import verify_build
baseline=module('container_baseline_verify',ROOT/'tools/parity/fixtures/alpine-preparation/verify.py')
SOURCES={'capture.py','inputs.json','guest.sh','guest.py','verify.py','support.py','verify_build.py','namespace.sh','guard-failure.sh','module-wait.sh','test_verify.py'}
COMMON=['br_netfilter','overlay','xt_comment','xt_conntrack','xt_MASQUERADE','xt_addrtype','xt_multiport','xt_nat']
BACKENDS={'Present(NfTables)':['nft_compat','nft_numgen','nft_redir','nft_limit','nft_tproxy'],
          'Present(Legacy)':['ip_tables','iptable_filter','iptable_nat','nf_conntrack']}
def finite(value):
    number=float(value);require(math.isfinite(number),'finite cloud timestamp');return number

def vm_load(path):
    return json.loads(read(path),object_pairs_hook=pairs,parse_constant=reject,parse_float=finite)

def equal(left,right,message):
    require(json.dumps(left,sort_keys=True)==json.dumps(right,sort_keys=True),message)

def mount_table(raw):
    require(len(raw.encode())<=1048576 and raw.endswith('\n') and raw.isascii(),'complete bounded mount table')
    rows={}
    for line in raw.splitlines():
        fields=line.split(' ');require(len(line)<=16384 and 10<=len(fields)<=128 and all(fields),'mount fields')
        require(fields.count('-')==1,'mount separator');sep=fields.index('-')
        require(sep>=6 and len(fields)==sep+4,'mount shape')
        require(re.fullmatch(r'[0-9]+',fields[0]) and re.fullmatch(r'[0-9]+',fields[1]),'mount ids')
        identity=int(fields[0]);require(identity>0 and identity not in rows,'unique mount id')
        def path(value):
            require(value.startswith('/') and '\x00' not in value,'absolute mount path')
            require(not re.search(r'\\(?!040|011|012|134)',value),'kernel mount escapes')
            return re.sub(r'\\(040|011|012|134)',lambda m:chr(int(m[1],8)),value)
        optional=fields[6:sep]
        shared=[v for v in optional if v.startswith('shared:')]
        require(len(shared)<=1 and all(re.fullmatch(r'shared:[1-9][0-9]*',v) for v in shared),'shared field')
        rows[identity]={'parent':int(fields[1]),'device':fields[2],'root':path(fields[3]),'point':path(fields[4]),
                        'shared':bool(shared),'private':'unbindable' not in optional and not any(v.startswith(('shared:','master:','propagate_from:')) for v in optional),'fs':fields[sep+1],'options':fields[5].split(',')}
    require(0<len(rows)<=4096,'mount record budget')
    roots=[key for key,row in rows.items() if row['point']=='/'];require(len(roots)==1,'one namespace root mount')
    for key in rows:
        seen=set();current=key
        while current!=roots[0]:
            require(current in rows and current not in seen and len(seen)<256,'complete acyclic bounded mount tree')
            seen.add(current);current=rows[current]['parent']
    return rows


CONTROLS=[f'/proc/sys/net/ipv6/conf/{name}/disable_ipv6' for name in ('all','default','lo')]
CASES=['correct','needs_write','guard','cancel']

def integer(value,minimum=1,maximum=2**64-1):
    return type(value) is int and minimum<=value<=maximum

def identity(value):
    require(set(value)=={'pid','starttime','exe_sha256','mnt','cgroup'},'identity fields')
    require(integer(value['pid'],maximum=2**32-1) and integer(value['starttime']),'live PID/starttime')
    require(re.fullmatch('[0-9a-f]{64}',value['exe_sha256']) is not None,'executable digest')
    require(isinstance(value['cgroup'],str) and len(value['cgroup'])<8192 and re.fullmatch(r'0::/[^\n]*\n',value['cgroup']) is not None,'v2 membership record')
    require(re.fullmatch(r'mnt:\[[0-9]+\]',value['mnt']) is not None,'mount namespace identity')

def file_identity(value):
    require(set(value)=={'device','inode','mode','uid','gid'},'file identity fields')
    require(all(integer(x,minimum=0) for x in value.values()) and value['inode']>0,'file identity values')

def external(value):
    import stat
    identity({k:value[k] for k in ['pid','starttime','exe_sha256','mnt','cgroup']})
    require(set(value)=={'pid','starttime','exe_sha256','mnt','cgroup','socket','configuration'},'external fields')
    file_identity(value['socket']);require(stat.S_ISSOCK(value['socket']['mode']),'live external Unix socket')
    require(set(value['configuration'])=={'identity','sha256'},'configuration fields')
    file_identity(value['configuration']['identity'])
    require(stat.S_ISREG(value['configuration']['identity']['mode']),'external configuration regular')
    require(re.fullmatch('[0-9a-f]{64}',value['configuration']['sha256']) is not None,'external config digest')

def config(text):
    lines=text.splitlines();sections={};current=None
    require(lines.count('apiVersion: kubesolo.io/v1alpha1')==1 and lines.count('kind: Config')==1,'configuration identity')
    for line in lines:
        if line and not line[0].isspace():
            current=line[:-1] if re.fullmatch('[A-Za-z][A-Za-z0-9]*:',line) else None
            if current is not None:
                require(current not in sections,'duplicate configuration section');sections[current]=[]
        elif current is not None:sections[current].append(line)
    for section,line in [('network','  disableIPv6: true'),('runtime','  containerMode: false'),('runtime','  endpoint: unix:///tmp/external-runtime/containerd.sock')]:
        require(lines.count(line)==1 and sections.get(section,[]).count(line)==1,'effective constrained guest flag')

def container_empty(value,stopped):
    equal(value,dict(status='NotStarted' if stopped else 'NotRequested',layout=None,mount=None,init=None,migration=None,
          available=[],enabled_before=[],enabled_after=None,missing_after=[],attempts=[],shared_effects_possible=False),'no container effects')

def result(value,pid,case,number):
    require(set(value)=={'schema','event','pass','pid','status','shared_effects_possible','network','container'},'result fields')
    require(type(value['schema']) is int and value['schema']==1 and value['event']=='result' and type(value['pass']) is int and value['pass']==number and value['pid']==pid,'typed result identity')
    status={'guard':'GuardStopped','cancel':'Cancelled'}.get(case,'Completed')
    require(value['status']==status and value['shared_effects_possible'] is (case!='guard'),'preparation disposition/effect latch')
    container_empty(value['container'],case in ['guard','cancel'])
    net=value['network'];require(set(net)=={'status','assessment','runtime','family','modules','ipv6'},'network fields')
    require(net['status']==status and net['runtime']=='External','network disposition and ownership')
    require(net['assessment']==('Unknown' if case=='guard' else 'Observed'),'fresh guard result')
    if case=='guard':
        require(net['modules']==[] and net['ipv6']==[] and net['family']=='Unknown(Io)','guard suppresses later effects without guessing backend');return
    require(net['family']=='Present(NfTables)','pinned nft userspace module family; no nft-only kernel claim')
    expected=COMMON+BACKENDS[net['family']]
    if case=='cancel':expected=expected[:1]
    require([x['name'] for x in net['modules']]==expected,'exact fixed ordered module attempts')
    for step in net['modules']:
        require(set(step)=={'name','outcome','spawned','joined','reaped','ownership_lost','exit','after'},'module fields')
        require(all(step[k] is True for k in ['spawned','joined','reaped']) and step['ownership_lost'] is False,'settled owned command')
        if case=='cancel':require(step['outcome']=='Cancelled' and step['after'] is None,'cancelled module with no later observation')
        else:
            require(step['outcome']=='Success' and type(step['exit']) is int and step['exit']==0,'real pinned module attempt')
            require(isinstance(step['after'],dict) and set(step['after'])=={'loaded','builtin_index','available_index'},'separate module observations')
            require(all(isinstance(x,str) and (x in ['Present(true)','Present(false)','Absent'] or re.fullmatch(r'Unknown\([A-Za-z]+\)',x)) for x in step['after'].values()),'typed module observations')
    if case=='cancel':require(net['ipv6']==[],'no post-cancellation sysctls');return
    require([x.get('path') for x in net['ipv6']]==CONTROLS and all(set(x)=={'path','outcome'} for x in net['ipv6']),'all three IPv6 controls')
    outcome='AlreadyDisabled' if case=='correct' else 'WriteFailed(Io)'
    require([x['outcome'] for x in net['ipv6']]==[outcome]*3,'read-only outcome with no false write success')

def semantic(text,metadata):
    require(len(text.encode())<=8*1024*1024 and text.endswith('\n'),'complete bounded guest records')
    rows=[loads(line) for line in text.splitlines()];cursor=0
    def take(event,case=None):
        nonlocal cursor
        require(cursor<len(rows),'missing event '+event);row=rows[cursor];cursor+=1
        require(type(row.get('schema')) is int and row['schema']==1 and row.get('event')==event,'ordered typed event '+event)
        if case is not None:require(row.get('case')==case,'case chronology')
        return row
    setup=take('setup');require(set(setup)=={'schema','event','launcher_argv','unshare_version','unshare_sha256','candidate_sha256','external'},'setup fields')
    pin=load(HERE/'inputs.json')['namespace_launcher']
    equal(setup['launcher_argv'],[pin['program'],*pin['argv']],'actual no-fork namespace argv')
    require(pin['argv']==['--mount','--propagation','private'] and pin['fork'] is False and pin['pid_namespace'] is False and pin['cgroup_namespace'] is False,'explicit namespace policy')
    require(setup['unshare_version']==pin['version'] and setup['unshare_sha256']==pin['sha256'],'pinned executed namespace tool')
    candidate=metadata['files']['prepare_node_host']['sha256'];require(setup['candidate_sha256']==candidate,'executed approved candidate')
    external(setup['external']);keeper=setup['external']
    cli=take('cli');require(set(cli)=={'schema','event','argv','exit','stdout','stderr'},'CLI fields')
    equal(cli['argv'],['--no-container-mode','--disable-ipv6','--container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock','--print-config'],'exact guest grammar')
    require(type(cli['exit']) is int and cli['exit']==0 and cli['stderr']=='','effect-free CLI');config(cli['stdout'])
    for case in CASES:
        value=1 if case=='correct' else 0
        before=take('before',case);require(set(before)=={'schema','event','case','external','sentinels','observer_mnt','root_enabled','observer_mounts','outside_values'},'before fields')
        equal(before['external'],keeper,'same live external resource before case')
        equal(before['outside_values'],[value]*3,'explicit fixture initial scalar state')
        require(set(before['sentinels'])=={'/etc/init.d','/etc/cni/net.d','/tmp/external-runtime'},'service/CNI/runtime sentinel scope')
        mount_table(before['observer_mounts'])
        observed=[]
        phases=['READY'] if case in ['guard','cancel'] else ['READY','FIRST','SECOND']
        for phase in phases:
            row=take('observation',case)
            require(set(row)=={'schema','event','case','phase','identity','external','sentinels','observer_mnt','root_enabled','observer_mounts','mounts','visible_values','outside_values'} and row['phase']==phase,'observation shape and phase')
            identity(row['identity']);require(row['identity']['exe_sha256']==candidate,'live candidate executable')
            require(row['identity']['mnt']!=row['observer_mnt'] and row['identity']['pid']!=keeper['pid'],'candidate isolated from keeper/observer')
            equal(row['identity']['cgroup'],keeper['cgroup'],'candidate cgroup membership untouched')
            for key in ['external','sentinels','observer_mnt','root_enabled','observer_mounts','outside_values']:equal(row[key],before[key],'outside resources unchanged at barrier '+key)
            equal(row['visible_values'],[value]*3,'independent read-only scalar readback')
            table=mount_table(row['mounts']);require(all(m['private'] for m in table.values()),'candidate mounts remain recursively private')
            sysctls=[m for m in table.values() if m['point']=='/proc/sys'];require(len(sysctls)==1 and sysctls[0]['fs']=='proc','actual proc sys bind mount')
            require(all('ro' in m['options'] for m in table.values() if m['point']=='/proc/sys' or m['point'].startswith('/proc/sys/')),'read-only sysctl mount and descendants')
            if observed:
                equal(row['identity'],observed[0]['identity'],'same live candidate PID/starttime/namespace/executable')
                equal(row['mounts'],observed[0]['mounts'],'candidate topology unchanged across preparation')
            observed.append(row)
        pid=observed[0]['identity']['pid'];double=None
        if case=='cancel':
            double=take('double_started',case);require(set(double)=={'schema','event','case','module','identity'} and double['module']=='br_netfilter','first module double marker');identity(double['identity'])
            require(double['identity']['pid'] not in [pid,keeper['pid']] and double['identity']['mnt']==observed[0]['identity']['mnt'],'owned double context')
        consumer=take('consumer',case)
        require(set(consumer)=={'schema','event','case','pid','exit','events','stderr'} and consumer['pid']==pid and consumer['stderr']=='','consumer receipt')
        require(type(consumer['exit']) is int and consumer['exit']==(1 if case in ['guard','cancel'] else 0),'consumer exit')
        events=consumer['events'];require(len(events)==(2 if case in ['guard','cancel'] else 6),'exact consumer event count')
        equal(events[0],dict(schema=1,event='READY',pid=pid),'READY before effects')
        result(events[1],pid,case,1)
        if case not in ['guard','cancel']:
            equal(events[2],dict(schema=1,event='FIRST',pid=pid),'first barrier');result(events[3],pid,case,2)
            equal(events[4],dict(schema=1,event='SECOND',pid=pid),'repeat barrier');equal(events[5],dict(schema=1,event='DONE',pid=pid),'complete protocol')
        elif case=='guard':
            row=take('guard_calls');equal(row,dict(schema=1,event='guard_calls',value='GUARD_DOUBLE\n'),'single explicitly labeled failed guard')
        else:equal(take('double_absent'),dict(schema=1,event='double_absent',pid=double['identity']['pid']),'cancelled owned double absent')
        after=take('after',case);require(set(after)==set(before)|{'pid_absent'},'after fields')
        require(after['pid_absent']==pid,'candidate reaped')
        for key in ['external','sentinels','observer_mnt','root_enabled','observer_mounts','outside_values']:equal(after[key],before[key],'outside resources unchanged after return '+key)
    equal(take('complete'),dict(schema=1,event='complete',keeper_pid_absent=keeper['pid'],socket_removed=True),'owned keeper orderly cleanup')
    require(cursor==len(rows),'no extra or omitted records')
    return dict(cases=CASES,real_passes=4,read_only=True,external_preserved=True,nft_only_kernel_qualified=False)

def verify_guest(directory):
    report=vm_load(directory/'result.json')
    require(report.get('status')=='passed' and report.get('errors')==[],'successful guest receipt')
    for key in ['owned_process_group_absent','owned_temporary_directory_removed','explicit_privilege_opt_in']:
        require(report.get(key) is True,'owned cleanup '+key)
    require(type(report.get('qemu_exit_code')) is int and report['qemu_exit_code']==0,'QEMU exit')
    require(report.get('shutdown')=='guest-poweroff' and report.get('privileged_mount_probe')=='passed','owned guest lifecycle')
    require(re.fullmatch('[a-f0-9]{40}',report.get('revision','')) is not None,'capture revision')
    require(report.get('working_tree_snapshot') is False,'frozen source')
    require(report.get('adapter')=='qemu-disposable-node-constrained','adapter')
    require(report.get('source_sha256')=={name:digest(HERE/name) for name in SOURCES},'current harness sources')
    require(report.get('baseline_helper_sha256')==digest(ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),'executed cleanup helper')
    require(report.get('inherited_vm_sha256')==digest(ROOT/'tools/parity/vm/run.py'),'VM lifecycle source')
    require(report.get('build_verifier_sha256')==digest(HERE/'verify_build.py') and report.get('build_support_sha256')==digest(HERE/'support.py'),'build verifier source')
    equal(report.get('approved_build_sources'),{name:digest(ROOT/'tools/node-container'/name) for name in ('verify_build.py','support.py','Build.Dockerfile','build.py','test-inventory.json')},'approved builder source bindings')
    equal(report['inputs'],load(HERE/'inputs.json'),'accepted VM pins')
    equal({name:tool['sha256'] for name,tool in report['tools'].items()},report['inputs']['host_tools'],'exact host tools')
    equal(report['firmware'],report['inputs']['firmware'],'exact firmware')
    metadata=verify_build.verify(directory/'artifact-build',binary=False)
    require(report.get('artifact_sha256')==metadata['files']['prepare_node_host']['sha256'],'guest candidate binding')
    cloud=vm_load(directory/'cloud-init.stdout');equal(report['cloud_init'],cloud,'raw cloud bootstrap')
    baseline.verify_cloud(report['cloud_init_exit'],cloud)
    require(report.get('serial_log_truncated',False) is False,'complete bounded console')
    private=report['owned_temporary_directory'];require(re.fullmatch(r'/tmp/rubix-vm-[a-zA-Z0-9_]+',private) is not None,'owned private path')
    port=report['ssh_forward'];require(re.fullmatch(r'127\.0\.0\.1:[0-9]+',port) is not None,'loopback only')
    expected=['qemu-system-aarch64','-machine','virt,accel=hvf','-cpu','host','-smp','2','-m','2048',
      '-display','none','-serial','stdio','-monitor','none','-qmp','unix:'+private+'/qmp.sock,server=on,wait=off',
      '-drive','if=pflash,format=raw,readonly=on,file=/opt/homebrew/share/qemu/edk2-aarch64-code.fd',
      '-drive','if=pflash,format=raw,file='+private+'/vars.fd','-drive','if=virtio,format=qcow2,file='+private+'/disk.qcow2',
      '-drive','if=virtio,format=raw,readonly=on,file='+private+'/seed.iso','-netdev',
      'user,id=n0,restrict=on,hostfwd=tcp:'+port+'-:22','-device','virtio-net-pci,netdev=n0']
    equal(report['qemu_argv'],expected,'exact owned resources and restricted network')
    require(re.fullmatch(r'ssh-ed25519 [A-Za-z0-9+/=]+ rubix-alpine-fixture-host',report.get('guest_host_public_key','')) is not None,'fixture-only host key')
    checked = read(directory/'verify-guest-inputs.stdout').decode().splitlines()
    names = {'guest.py','namespace.sh','guard-failure.sh','module-wait.sh',*verify_build.BINARIES,'repo/aarch64/APKINDEX.tar.gz'}
    names.update('repo/aarch64/'+name for name in report['inputs']['packages']['selected'])
    require(checked==[name+': OK' for name in sorted(names)],'guest verified every exact input')
    require(read(directory/'verify-guest-inputs.stderr')==b'', 'guest input verification stderr')
    observation=read(directory/'constrained-cases.stdout').decode()
    require(report.get('observation')==observation,'raw guest observations')
    return semantic(observation,metadata)

def verify(directory):
    provenance=load(directory/'provenance.json')
    require(set(provenance)=={'files'},'provenance shape')
    paths={str(p.relative_to(directory/'evidence')) for p in (directory/'evidence').rglob('*') if p.is_file()}
    require(paths==set(provenance['files']),'exact raw evidence inventory')
    for name,value in provenance['files'].items():
        require(not Path(name).is_absolute() and '..' not in Path(name).parts,'safe evidence path')
        raw=read(directory/'evidence'/name,32*1024*1024)
        require(hashlib.sha256(raw).hexdigest()==value,'raw file binding')
        require(b'BEGIN OPENSSH PRIVATE KEY' not in raw,'no private credential evidence')
    first=verify_guest(directory/'evidence/first');repeat=verify_guest(directory/'evidence/repeat')
    equal(first,repeat,'independent guest repeated semantics')
    return first
if __name__=='__main__':
    verify(Path(sys.argv[1]) if len(sys.argv)>1 else HERE);print('constrained guest evidence verified')
