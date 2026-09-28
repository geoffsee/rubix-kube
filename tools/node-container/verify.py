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
SOURCES={'capture.py','inputs.json','guest.sh','verify.py','support.py','launch.sh','namespace.sh'}
COMMON=['br_netfilter','overlay','xt_comment','xt_conntrack','xt_MASQUERADE','xt_addrtype','xt_multiport','xt_nat']
BACKENDS={'Present(NfTables)':['nft_compat','nft_numgen','nft_redir','nft_limit','nft_tproxy'],
          'Present(Legacy)':['ip_tables','iptable_filter','iptable_nat','nf_conntrack']}
REQUIRED={'cpuset','cpu','io','memory','pids'}

def finite(value):
    number=float(value);require(math.isfinite(number),'finite cloud timestamp');return number

def vm_load(path):
    return json.loads(read(path),object_pairs_hook=pairs,parse_constant=reject,parse_float=finite)

def equal(left,right,message):
    require(json.dumps(left,sort_keys=True)==json.dumps(right,sort_keys=True),message)

def block(text,name):
    begin=name+'_BEGIN\n';end=name+'_END\n'
    require(text.count(begin)==1 and text.count(end)==1,'unique complete '+name)
    before,tail=text.split(begin);body,after=tail.split(end)
    return body,before+after

def scalar(text,name,pattern):
    rows=re.findall('^'+name+' ('+pattern+')$',text,re.M)
    require(len(rows)==1,'unique '+name)
    return rows[0],re.sub('^'+name+' '+pattern+'\n','',text,count=1,flags=re.M)

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
                        'shared':bool(shared),'private':'unbindable' not in optional and not any(v.startswith(('shared:','master:','propagate_from:')) for v in optional),'fs':fields[sep+1]}
    require(0<len(rows)<=4096,'mount record budget')
    roots=[key for key,row in rows.items() if row['point']=='/'];require(len(roots)==1,'one namespace root mount')
    for key in rows:
        seen=set();current=key
        while current!=roots[0]:
            require(current in rows and current not in seen and len(seen)<256,'complete acyclic bounded mount tree')
            seen.add(current);current=rows[current]['parent']
    return rows

def topology(rows):
    return {key:{k:v for k,v in row.items() if k not in {'shared','private'}} for key,row in rows.items()}

def names(raw):
    require(len(raw)<=4096,'controller bytes');tokens=raw.split()
    require(len(tokens)<=32 and len(tokens)==len(set(tokens)) and all(re.fullmatch('[a-z][a-z0-9_]{0,63}',v) for v in tokens),'controller set')
    return set(tokens)

def pids(raw):
    require(len(raw)<=65536,'PID bytes');tokens=raw.split()
    require(len(tokens)<=4096 and all(re.fullmatch('[1-9][0-9]{0,9}',v) for v in tokens),'PID list')
    require(all(int(v)<=4294967295 for v in tokens),'PID u32 range')
    return [int(v) for v in tokens]

def observation(body,phase,artifact):
    values={};remaining=body
    for name,pattern in [('PID',r'[1-9][0-9]*'),('EXE',r'[a-f0-9]{64}  /proc/[1-9][0-9]*/exe'),
                         ('MNT_NS',r'mnt:\[[0-9]+\]'),('CGROUP_NS',r'cgroup:\[[0-9]+\]'),
                         ('OBSERVER_MNT_NS',r'mnt:\[[0-9]+\]'),('OBSERVER_CGROUP_NS',r'cgroup:\[[0-9]+\]'),
                         ('GLOBAL_ROOT_ID',r'[0-9]+:[0-9]+'),('ROOT_ID',r'[0-9]+:[0-9]+'),('VISIBLE_ROOT_ID',r'[0-9]+:[0-9]+')]:
        values[name],remaining=scalar(remaining,name,pattern)
    for name in ['STAT','CANDIDATE_MOUNTS','OBSERVER_MOUNTS','PROCESS_CGROUP','ROOT_TYPE','ROOT_MEMBERS','AVAILABLE','ROOT_ENABLED','PARENT_ENABLED','SIBLING','EXTERNAL']:
        values[name],remaining=block(remaining,name)
    pid=int(values['PID']);require(values['EXE']==artifact+'  /proc/'+str(pid)+'/exe','independent candidate executable')
    stat=values['STAT'].rstrip('\n');close=stat.rfind(') ')
    require(close>0 and stat.startswith(str(pid)+' ('),'process stat identity')
    fields=stat[close+2:].split();require(len(fields)>=20 and fields[0] not in {'Z','X'} and fields[19].isdigit(),'live process starttime')
    values['starttime']=int(fields[19]);require(values['starttime']>0,'starttime positive')
    require(values['MNT_NS']!=values['OBSERVER_MNT_NS'] and values['CGROUP_NS']!=values['OBSERVER_CGROUP_NS'],'isolated namespaces')
    require(values['ROOT_ID']==values['VISIBLE_ROOT_ID'] and values['ROOT_ID']!=values['GLOBAL_ROOT_ID'],'anchored delegated nonroot bind')
    require(values['ROOT_TYPE']=='domain\n','domain delegated root')
    available=names(values['AVAILABLE']);require(REQUIRED<=available,'nonempty required available controllers')
    require(available<=names(values['PARENT_ENABLED']),'parent fixture delegation')
    mounts=mount_table(values['CANDIDATE_MOUNTS']);mount_table(values['OBSERVER_MOUNTS'])
    require(any(row['point']=='/sys/fs/cgroup' and row['fs']=='cgroup2' for row in mounts.values()),'visible cgroup2 mount')
    if phase.endswith('READY'):
        require(all(row['private'] for row in mounts.values()),'private before any candidate effect')
        require(pids(values['ROOT_MEMBERS'])==[pid] and not names(values['ROOT_ENABLED']),'initial root membership/empty delegation')
        require(remaining=='INIT_ABSENT\n','initial init absent')
        expected='/rubix-container-'+phase.split('_')[0]
    else:
        for name in ['INIT_MEMBERS','INIT_TYPE','INIT_ENABLED']:
            values[name],remaining=block(remaining,name)
        require(remaining=='','observation field inventory')
        require(pids(values['ROOT_MEMBERS'])==[] and pids(values['INIT_MEMBERS'])==[pid],'actual process root to init migration')
        require(values['INIT_TYPE']=='domain\n' and not names(values['INIT_ENABLED']),'init domain and no child delegation')
        require(names(values['ROOT_ENABLED'])==available,'independent positive delegation')
        require(all(row['shared'] for row in mounts.values()),'all recursive mounts observed shared')
        expected='/rubix-container-real/init'
    require(values['PROCESS_CGROUP']=='0::'+expected+'\n','outside process membership path')
    sibling=values['SIBLING'].splitlines()
    expected_files=['cgroup.type','cgroup.procs','cgroup.subtree_control','pids.max']
    require(len(sibling)==4,'sibling inventory')
    for line,name in zip(sibling,expected_files):
        require(re.fullmatch('[a-f0-9]{64}  /sys/fs/cgroup/rubix-container-sibling/'+re.escape(name),line),'sibling binding')
    for name,text in [('config.toml','host-owned configuration sentinel\n'),('state','host-owned state sentinel\n')]:
        require(values['EXTERNAL'].splitlines().count(hashlib.sha256(text.encode()).hexdigest()+'  /tmp/external-runtime/'+name)==1,'external sentinel')
    require(len(values['EXTERNAL'].splitlines())==2,'external inventory')
    values['mounts']=mounts;values['pid']=pid;values['available']=available
    return values

def consumer(body,scenario):
    code,body=scalar(body,'EXIT','[01]');out,body=block(body,'STDOUT');err,body=block(body,'STDERR')
    require(body=='' and err=='','consumer framing/stderr')
    pid,out=scalar(out,'LAUNCH_PID','[1-9][0-9]*');private,out=block(out,'PRIVATE_BEFORE_BIND')
    require(all(row['private'] for row in mount_table(private).values()),'recursive private before bind')
    events=[loads(line) for line in out.splitlines()];require(all(type(v) is dict and type(v.get('schema')) is int and v['schema']==1 for v in events),'event objects')
    expected=['READY','protocol_failure'] if scenario=='signal' else ['READY','result','FIRST','result','SECOND','DONE']
    require([v.get('event') for v in events]==expected,'ordered protocol events')
    for event in events:
        if event['event'] in {'READY','FIRST','SECOND','DONE'}:
            equal(event,dict(schema=1,event=event['event'],pid=int(pid)),'barrier fields/PID')
    if scenario=='signal':
        require(code=='1','signal exit')
        equal(events[1],dict(schema=1,event='protocol_failure',phase='READY',reason='cancelled',preparation_started=False,shared_effects_possible=False),'pre-G signal suppresses preparation')
    else: require(code=='0','real exit')
    return int(pid),events

def result(value,pass_number,observed):
    require(set(value)=={'schema','event','pass','pid','status','shared_effects_possible','network','container'},'result fields')
    require(type(value['pass']) is int and value['pass']==pass_number and type(value['pid']) is int and value['pid']==observed['pid'],'pass/PID')
    require(value['status']=='Completed' and value['shared_effects_possible'] is True,'completed partial-effect report')
    network=value['network'];require(set(network)=={'status','assessment','runtime','family','modules','ipv6'},'network fields')
    require(network['status']=='Completed' and network['assessment']=='Observed' and network['runtime']=='External' and network['family'] in BACKENDS,'fresh network guards/external ownership')
    require(network['ipv6']==[],'unrequested IPv6')
    rows=network['modules'];require([v.get('name') for v in rows]==COMMON+BACKENDS[network['family']],'ordered actual modules')
    for row in rows:
        require(set(row)=={'name','outcome','spawned','joined','reaped','ownership_lost','exit','after'},'module fields')
        require(all(row[key] is True for key in ['spawned','joined','reaped']) and row['ownership_lost'] is False,'settled module owner')
        require(row['outcome']=='Success' and type(row['exit']) is int and row['exit']==0,'real command settled successfully, not proof of loaded state')
        require(type(row['after']) is dict and set(row['after'])=={'loaded','builtin_index','available_index'},'independent module observations')
        require(all(type(v) is str and re.fullmatch(r'Present\((true|false)\)|Absent|Unknown\([A-Za-z]+\)',v) for v in row['after'].values()),'typed module observations')
    container=value['container'];require(set(container)=={'status','layout','mount','init','migration','available','enabled_before','enabled_after','missing_after','attempts','shared_effects_possible'},'container fields')
    require(container['status']=='Completed' and container['layout']=='V2' and container['shared_effects_possible'] is True,'real container completion')
    for name,attempted in [('mount',True),('init',pass_number==1),('migration',True)]:
        equal(container[name],dict(attempted=attempted,result='Ok(())'),'actual '+name)
    available=observed['available']
    require(type(container['available']) is list and len(container['available'])==len(available) and set(container['available'])==available,'available report/readback')
    before=[] if pass_number==1 else container['available']
    require(type(container['enabled_before']) is list and len(container['enabled_before'])==len(before) and set(container['enabled_before'])==set(before),'initial enabled report')
    require(type(container['enabled_after']) is dict and set(container['enabled_after'])=={'names'},'observed delegation report')
    require(set(container['enabled_after']['names'])==available and len(container['enabled_after']['names'])==len(available),'enabled report matches observer')
    require(container['missing_after']==[] and len(container['attempts'])==1,'real successful bulk only')
    equal(container['attempts'][0],dict(controllers=container['available'],mutation=dict(attempted=True,result='Ok(())')),'bulk request report')
    return network['family']

def semantic(text,metadata):
    require(len(text.encode())<=8*1024*1024,'guest output bound')
    remaining=text;setup,remaining=block(remaining,'SETUP')
    pin=load(HERE/'inputs.json')['namespace_launcher']
    require(setup.splitlines().count(pin['version'])==1 and setup.splitlines().count('UNSHARE_SHA256 '+pin['sha256']+'  '+pin['program'])==1,'actual pinned namespace launcher')
    for name in ['help','version','print']:
        body,remaining=block(remaining,'CLI_'+name);code,body=scalar(body,'EXIT','0');out,body=block(body,'STDOUT');err,body=block(body,'STDERR');require(body=='','CLI framing')
        if name=='help': require(out=='' and err==read(ROOT/'crates/rubix-kube/src/help.txt').decode(),'complete effect-free help')
        elif name=='version': require(out=='' and loads(err)==dict(level='info',message='kubesolo version',version='0.1.0'),'version')
        else: require(err=='','print stderr');verify_build.verify_config(out)
    tests,remaining=block(remaining,'INJECTED_TESTS')
    verify_build.verify_tests(tests)
    for mode,reason in [('eof','eof'),('wrong','wrong_byte')]:
        body,remaining=block(remaining,'PROTOCOL_'+mode);code,body=scalar(body,'EXIT','1');err,body=block(body,'STDERR');require(err=='','protocol stderr')
        events=[loads(line) for line in body.splitlines()]
        require(len(events)==2 and set(events[0])=={'schema','event','pid'} and events[0]['event']=='READY' and type(events[0]['schema']) is int and events[0]['schema']==1 and type(events[0]['pid']) is int and events[0]['pid']>0,'protocol READY')
        equal(events[1],dict(schema=1,event='protocol_failure',phase='READY',reason=reason,preparation_started=False,shared_effects_possible=False),'protocol failure without effects')
    observations={}
    for phase in ['signal_READY','real_READY','real_FIRST','real_SECOND']:
        body,remaining=block(remaining,'OBS_'+phase)
        observations[phase]=observation(body,phase,metadata['files']['prepare_node_host']['sha256'])
    outputs={}
    for scenario in ['signal','real']:
        body,remaining=block(remaining,'CONSUMER_'+scenario);outputs[scenario]=consumer(body,scenario)
        require(outputs[scenario][0]==observations[scenario+'_READY']['pid'],'launcher/candidate same PID')
    reference=observations['signal_READY']
    for observed in observations.values():
        for key in ['OBSERVER_MNT_NS','OBSERVER_CGROUP_NS','OBSERVER_MOUNTS','PARENT_ENABLED','SIBLING','EXTERNAL','GLOBAL_ROOT_ID']:
            require(observed[key]==reference[key],'outside scope unchanged '+key)
    real=observations['real_READY'];family=None
    for pass_number,phase in [(1,'real_FIRST'),(2,'real_SECOND')]:
        observed=observations[phase]
        for key in ['pid','starttime','EXE','MNT_NS','CGROUP_NS','ROOT_ID','VISIBLE_ROOT_ID']:
            require(observed[key]==real[key],'same live process/namespace/root '+key)
        require(topology(observed['mounts'])==topology(real['mounts']),'same anchored mount topology')
        family=result(outputs['real'][1][2*pass_number-1],pass_number,observed)
    require(remaining=='SIGNAL_NO_PREPARATION\nPID_ABSENT '+str(reference['pid'])+'\nREAL_PROCESS_EXITED\nPID_ABSENT '+str(real['pid'])+'\nCONTAINER_GUEST_COMPLETE\n','complete final cleanup records')
    order=['SETUP_BEGIN','CLI_help_BEGIN','CLI_version_BEGIN','CLI_print_BEGIN','INJECTED_TESTS_BEGIN','PROTOCOL_eof_BEGIN','PROTOCOL_wrong_BEGIN','OBS_signal_READY_BEGIN','CONSUMER_signal_BEGIN','OBS_real_READY_BEGIN','OBS_real_FIRST_BEGIN','OBS_real_SECOND_BEGIN','CONSUMER_real_BEGIN']
    require([text.index(value) for value in order]==sorted(text.index(value) for value in order),'event chronology')
    return dict(passes=2,controllers=sorted(real['available']),family=family,injected_tests=38)

def verify_guest(directory):
    report=vm_load(directory/'result.json')
    require(report.get('status')=='passed' and report.get('errors')==[],'successful guest receipt')
    for key in ['owned_process_group_absent','owned_temporary_directory_removed','explicit_privilege_opt_in']:
        require(report.get(key) is True,'owned cleanup '+key)
    require(type(report.get('qemu_exit_code')) is int and report['qemu_exit_code']==0,'QEMU exit')
    require(report.get('shutdown')=='guest-poweroff' and report.get('privileged_mount_probe')=='passed','owned guest lifecycle')
    require(re.fullmatch('[a-f0-9]{40}',report.get('revision','')) is not None,'capture revision')
    require(report.get('working_tree_snapshot') is False,'frozen source')
    require(report.get('adapter')=='qemu-disposable-node-container','adapter')
    require(report.get('source_sha256')=={name:digest(HERE/name) for name in SOURCES},'current harness sources')
    require(report.get('baseline_helper_sha256')==digest(ROOT/'tools/parity/fixtures/alpine-preparation/capture.py'),'executed cleanup helper')
    require(report.get('inherited_vm_sha256')==digest(ROOT/'tools/parity/vm/run.py'),'VM lifecycle source')
    require(report.get('build_verifier_sha256')==digest(HERE/'verify_build.py') and report.get('build_support_sha256')==digest(HERE/'support.py'),'build verifier source')
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
    names = {'launch.sh','namespace.sh',*verify_build.BINARIES,'repo/aarch64/APKINDEX.tar.gz'}
    names.update('repo/aarch64/'+name for name in report['inputs']['packages']['selected'])
    require(checked==[name+': OK' for name in sorted(names)],'guest verified every exact input')
    require(read(directory/'verify-guest-inputs.stderr')==b'', 'guest input verification stderr')
    observation=read(directory/'container-cases.stdout').decode()
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
    verify(Path(sys.argv[1]) if len(sys.argv)>1 else HERE);print('container guest evidence verified')
