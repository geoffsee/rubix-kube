"""Strict source-bound disposable-VM network outcomes; this verifier starts no processes."""
import hashlib
import json
import math
from pathlib import Path
import re
import sys
from support import HERE, ROOT, digest, load, loads, module, pairs, read, reject, require
import verify_build
baseline = module('network_baseline_verify', ROOT/'tools/parity/fixtures/alpine-preparation/verify.py')
SOURCES = {'capture.py','inputs.json','guest.sh','verify.py','support.py','modprobe-double.sh','iptables-double.sh'}
COMMON = ['br_netfilter','overlay','xt_comment','xt_conntrack','xt_MASQUERADE','xt_addrtype','xt_multiport','xt_nat']
BACKENDS = {'Present(NfTables)': ['nft_compat','nft_numgen','nft_redir','nft_limit','nft_tproxy'],
            'Present(Legacy)': ['ip_tables','iptable_filter','iptable_nat','nf_conntrack']}
CONTROLS = ['/proc/sys/net/ipv6/conf/'+name+'/disable_ipv6' for name in ['all','default','lo']]
CASES = ['help','version','print','guard_failed','double_failed','double_limits','double_cancel','real_first','real_repeat']

def finite(value):
    number=float(value);require(math.isfinite(number),'finite cloud timestamp');return number

def vm_load(path):
    # cloud-init timestamps and SSH boot elapsed times are finite numbers; all other
    # semantic integer/bool fields are checked by exact type below.
    return json.loads(read(path),object_pairs_hook=pairs,parse_constant=reject,parse_float=finite)

def equal(left,right,message):
    require(json.dumps(left,sort_keys=True)==json.dumps(right,sort_keys=True),message)

def cases(text):
    pattern=r'CASE_([a-z_]+)_BEGIN\nEXIT ([0-9]+)\nSTDOUT_BEGIN\n(.*?)STDOUT_END\nSTDERR_BEGIN\n(.*?)STDERR_END\nCASE_\1_END\n'
    rows=re.findall(pattern,text,re.S)
    require(len(rows)*2==text.count('CASE_'),'complete unique case frames')
    require([r[0] for r in rows]==CASES,'case order')
    require([int(r[1]) for r in rows]==[0,0,0,1,0,0,1,0,0],'case exit statuses')
    return rows

def outcome(raw):
    value=loads(raw)
    require(type(value) is dict and set(value)=={'schema','status','assessment','runtime','family','shared_effects_possible','modules','ipv6'},'consumer fields')
    require(type(value['schema']) is int and value['schema']==1,'consumer schema')
    require(type(value['shared_effects_possible']) is bool,'effect type')
    require(value['runtime']=='External','external runtime configured')
    require(type(value['modules']) is list and len(value['modules'])<=13,'module bound')
    require(type(value['ipv6']) is list and len(value['ipv6'])<=3,'IPv6 bound')
    return value

def module_rows(rows,names,observed):
    require(all(type(row) is dict for row in rows),'module object types')
    require([row.get('name') for row in rows]==names,'fixed ordered module attempts')
    for row in rows:
        require(set(row)=={'name','outcome','spawned','joined','reaped','ownership_lost','exit','after'},'module fields')
        for key in ['spawned','joined','reaped','ownership_lost']:
            require(type(row[key]) is bool,'cleanup boolean')
        require(row['spawned'] and row['joined'] and row['reaped'] and not row['ownership_lost'],'settled owned module command')
        require(row['exit'] is None or type(row['exit']) is int,'exit type')
        if observed:
            require(type(row['after']) is dict and set(row['after'])=={'loaded','builtin_index','available_index'},'post-command observations')
            for fact in row['after'].values():
                require(type(fact) is str and re.fullmatch(r'Present\((true|false)\)|Absent|Unknown\([A-Za-z]+\)',fact) is not None,'typed observation')
        else:
            require(row['after'] is None,'no post-cancel observation')

def scalar(text,name,value):
    line='SCALARS_'+name+' all='+value+' default='+value+' lo='+value+' '
    require(text.splitlines().count(line)==1,'independent sysctl readback '+name)

def attempts(text,name,names):
    begin='ATTEMPTS_'+name+'_BEGIN\n';end='ATTEMPTS_'+name+'_END\n'
    require(text.count(begin)==1 and text.count(end)==1,'attempt markers')
    require(text.split(begin)[1].split(end)[0].splitlines()==names,'double actual argv order')

def semantic(text):
    rows=cases(text)
    results={}
    for name,code,stdout,stderr in rows:
        if name=='help':
            require(stdout=='' and stderr.startswith('usage: kubesolo'),'effect-free help')
        elif name=='version':
            require(stdout=='' and loads(stderr)=={'level':'info','message':'kubesolo version','version':'0.1.0'},'effect-free version')
        elif name=='print':
            require(stderr=='' and stdout.startswith('apiVersion: kubesolo.io/v1alpha1\n'),'effect-free configuration')
        else:
            require(stderr=='','no unexpected consumer stderr')
            results[name]=outcome(stdout)
    guard=results['guard_failed']
    require(guard['status']=='GuardStopped' and guard['assessment']=='Unknown','failed backend guard')
    require(not guard['shared_effects_possible'] and guard['modules']==[] and guard['ipv6']==[],'guard suppresses all preparation')
    require(type(guard['family']) is str and guard['family'].startswith('Unknown('),'failed backend remains unknown')
    family=results['real_first']['family'];require(family in BACKENDS,'known module family')
    names=COMMON+BACKENDS[family]
    for name in ['double_failed','double_limits','real_first','real_repeat']:
        result=results[name]
        require(result['status']=='Completed' and result['assessment']=='Observed' and result['shared_effects_possible'] is True,'settled preparation '+name)
        require(result['family']==family,'consistent observed backend')
        module_rows(result['modules'],names,True)
        require(all(type(step) is dict for step in result['ipv6']),'IPv6 object types')
        require([step.get('path') for step in result['ipv6']]==CONTROLS,'all fixed IPv6 controls')
        for step in result['ipv6']:
            require(set(step)=={'path','outcome'},'IPv6 fields')
            require(step['outcome']==('AlreadyDisabled' if name=='real_repeat' else 'ObservedDisabled'),'actual IPv6 write/readback or idempotent skip')
        for step in result['modules']:
            require(step['outcome'] in ['Success','Failed','Deadline','CaptureFailed'],'ordinary settled outcome')
            if name=='double_failed':
                require(step['outcome']=='Failed' and step['exit']==17,'labeled failure-double outcome')
            if name=='double_limits':
                expected={'br_netfilter':'Deadline','overlay':'CaptureFailed'}.get(step['name'],'Failed')
                require(step['outcome']==expected,'adapter execution/output limits')
            if step['outcome']=='Success':require(step['exit']==0,'successful exit only; not proof loaded')
    cancel=results['double_cancel']
    require(cancel['status']=='Cancelled' and cancel['assessment']=='Observed' and cancel['family']==family,'cancelled double')
    require(cancel['shared_effects_possible'] is True and cancel['ipv6']==[],'cancel latch and no later writes')
    module_rows(cancel['modules'],COMMON[:1],False)
    require(cancel['modules'][0]['outcome']=='Cancelled','acknowledged module cancellation')
    attempts(text,'double_failed',names);attempts(text,'double_limits',names);attempts(text,'double_cancel',COMMON[:1])
    for name in ['initial','guard_failed','double_cancel']:scalar(text,name,'0')
    for name in ['double_failed','double_limits','real_first','real_repeat']:scalar(text,name,'1')
    for name in ['modprobe','iptables']:
        values={}
        for phase in ['ORIGINAL','RESTORED']:
            matches=re.findall(r'^'+phase+'_'+name+r' (ABSENT|[a-f0-9]{64}  /usr/sbin/'+name+r')$',text,re.M)
            require(len(matches)==1,'command restoration marker');values[phase]=matches[0]
        require(values['ORIGINAL']==values['RESTORED'],'original command restored')
        next_case='double_failed' if name=='iptables' else 'real_first'
        require(text.index('RESTORED_'+name+' ')<text.index('CASE_'+next_case+'_BEGIN'),'restore before genuine command execution')
        for kind,pattern in [('KIND',r'ABSENT|REGULAR|LINK [^\r\n]+'),('RESOLVED',r'[a-f0-9]{64}  /(?:usr/)?s?bin/[a-z_-]+')]:
            original=re.findall(r'^ORIGINAL_'+kind+'_'+name+' ('+pattern+')$',text,re.M)
            restored=re.findall(r'^RESTORED_'+kind+'_'+name+' ('+pattern+')$',text,re.M)
            require(len(original)==1 and original==restored,'restored original command type/link/resolution')
        expected='DOUBLE_'+name+' '+digest(HERE/(name+'-double.sh'))+'  /usr/sbin/'+name
        require(text.splitlines().count(expected)==1,'bound double bytes')
    for marker in ['SETUP_BEGIN','SETUP_END','MODULES_BEFORE_BEGIN','MODULES_BEFORE_END','MODULES_AFTER_BEGIN','MODULES_AFTER_END','EXTERNAL_SENTINEL_UNCHANGED','NETWORK_GUEST_COMPLETE']:
        require(text.splitlines().count(marker)==1,'complete guest observation '+marker)
    return {'family':family,'modules':names,'cases':CASES}

def verify_guest(directory):
    report=vm_load(directory/'result.json')
    require(report.get('status')=='passed' and report.get('errors')==[],'successful guest receipt')
    for key in ['owned_process_group_absent','owned_temporary_directory_removed','explicit_privilege_opt_in']:
        require(report.get(key) is True,'owned cleanup '+key)
    require(type(report.get('qemu_exit_code')) is int and report['qemu_exit_code']==0,'QEMU exit')
    require(report.get('shutdown')=='guest-poweroff' and report.get('privileged_mount_probe')=='passed','owned guest lifecycle')
    require(re.fullmatch('[a-f0-9]{40}',report.get('revision','')) is not None,'capture revision')
    require(report.get('working_tree_snapshot') is False,'frozen source')
    require(report.get('adapter')=='qemu-disposable-node-network','adapter')
    require(report.get('source_sha256')=={name:digest(HERE/name) for name in SOURCES},'current harness sources')
    require(report.get('baseline_helper_sha256')==digest(ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),'executed cleanup helper')
    require(report.get('inherited_vm_sha256')==digest(ROOT/'tools/parity/vm/run.py'),'VM lifecycle source')
    require(report.get('build_verifier_sha256')==digest(HERE/'verify_build.py') and report.get('build_support_sha256')==digest(HERE/'support.py'),'build verifier source')
    equal(report['inputs'],load(HERE/'inputs.json'),'accepted VM pins')
    equal({name:tool['sha256'] for name,tool in report['tools'].items()},report['inputs']['host_tools'],'exact host tools')
    equal(report['firmware'],report['inputs']['firmware'],'exact firmware')
    metadata=verify_build.verify(directory/'artifact-build',binary=False)
    require(report.get('artifact_sha256')==metadata['sha256'],'guest candidate binding')
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
    names = {'modprobe-double.sh','iptables-double.sh','prepare_host_network','repo/aarch64/APKINDEX.tar.gz'}
    names.update('repo/aarch64/'+name for name in report['inputs']['packages']['selected'])
    require(checked==[name+': OK' for name in sorted(names)],'guest verified every exact input')
    require(read(directory/'verify-guest-inputs.stderr')==b'', 'guest input verification stderr')
    observation=read(directory/'network-cases.stdout').decode()
    require(report.get('observation')==observation,'raw guest observations')
    return semantic(observation)

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
    verify(Path(sys.argv[1]) if len(sys.argv)>1 else HERE);print('network guest evidence verified')
