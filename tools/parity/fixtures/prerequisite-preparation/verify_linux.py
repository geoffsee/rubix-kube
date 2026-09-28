import pathlib,re,hashlib
from support import HERE,ROOT,require,equal,load,strict,read,digest
HARNESS=['qualify.py','Linux.Dockerfile','linux_fixture.py','verify_linux.py','support.py','fixture-command.rs','../management-check/capture.py','../management-check/verify.py']
NAMES=['help','version','root_pass','nonroot','preparation','pprof_off','pprof_conflict','repeat_0','repeat_1']
EXITS=[0,0,0,1,1,0,1,0,0]
def verify_run(raw):
 text=raw.decode();require(text.count('test result: ok. 6 passed;')==1 and text.count('test result: ok. 7 passed;')==2,'Rust test completion')
 binaries=re.findall(r'^([a-f0-9]{64})  /out/([a-z-]+)$',text,re.M)
 require(len({value for value,_ in binaries})==5 and len(binaries)==5 and sorted(name for _,name in binaries)==['check-tests','fixture-command','preparation-tests','rubixctl','rubixctl-tests'],'binary identity')
 records=[strict(line[len(b'RUBIX_PREPARATION '):]) for line in raw.splitlines() if line.startswith(b'RUBIX_PREPARATION ')]
 require(len(records)==1,'record count');record=records[0]
 require(type(record) is dict and set(record)=={'cases'},'record schema');rows=record['cases']
 names=['opt_out','prepare','repeat','no_effect_success','register_failure','nonroot']+['signal_'+str(i) for i in range(20)]
 require(type(rows) is list and len(rows)==len(names),'case count')
 apk='/sbin/apk add --no-cache nftables iptables';update='/sbin/rc-update add cgroups boot';start='/sbin/rc-service cgroups start'
 for row,name in zip(rows,names,strict=True):
  require(type(row) is dict and set(row)=={'exit','actions','signal_sent','owned_child_absent','stdout','stderr','name'},'case schema');equal(row['name'],name,'name')
  equal(row['exit'],0 if name in ['prepare','repeat'] else 1,'exit')
  equal(row['stdout'],'','stdout');require(type(row['stderr']) is str,'stderr')
  actions=[] if name in ['opt_out','nonroot'] else [apk,update,start] if name in ['prepare','repeat'] else [apk,update] if name=='register_failure' else [apk]
  equal(row['actions'],actions,'exact command order')
  signalled=name.startswith('signal_');equal(row['owned_child_absent'],signalled,'owned child cleanup')
  equal(row['signal_sent'],(2 if int(name[7:])%2==0 else 15) if signalled else None,'signal')
  if name in ['prepare','repeat']:require('All 7 checks passed' in row['stderr'],'success')
  else:require('All 7 checks passed' not in row['stderr'],'false success')
  if name in ['register_failure','no_effect_success'] or signalled:require('host state may have changed' in row['stderr'],'partial effects warning')
 return record

def relevant(name):
 return name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml'} or name.endswith('/Cargo.toml') or name.startswith('.cargo/') or name.startswith(('crates/rubix-platform/','crates/rubixctl/','crates/rubix-supervisor/')) and name.endswith('.rs') or name in {'tools/parity/fixtures/management-check/'+n for n in ['cases.json','expected.json']} or name in {'tools/parity/fixtures/prerequisite-preparation/'+n for n in ['linux_fixture.py','fixture-command.rs']}
def verify(directory):
 directory=pathlib.Path(directory);r=load(directory/'receipt.json')
 require(set(r)=={'revision','uncommitted_implementation','platform','target','containers','errors','cleanup_errors','helper_sha256','source_sha256','inventory_sha256','image_id','command','run_sha256','artifact_sha256','remaining_containers','remaining_images'},'receipt schema')
 require(type(r['revision']) is str and re.fullmatch('[a-f0-9]{40}',r['revision']) is not None,'revision');require(r['uncommitted_implementation'] is True,'working tree qualification')
 require(type(r['image_id']) is str and re.fullmatch('sha256:[a-f0-9]{64}',r['image_id']) is not None,'image identity')
 equal(r['platform'],'linux/arm64','platform');equal(r['target'],'aarch64-unknown-linux-musl','target')
 for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:equal(r[key],[],key)
 equal(r['source_sha256'],{n:digest(HERE/n) for n in HARNESS},'harness hashes');equal(r['helper_sha256'],digest(ROOT/'tools/defaults/capture.py'),'helper')
 require(type(r['containers']) is list and len(r['containers'])==1 and re.fullmatch('rubix-preparation-linux-[a-f0-9]{32}-test',r['containers'][0]) is not None,'owned container')
 tag=r['containers'][0][:-5]
 equal(r['command'],['docker','run','--name',tag+'-test','--hostname','fixture','--init','--read-only','--network','none','--cap-drop','ALL','--cap-add','SYS_CHROOT','--cap-add','SETUID','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,exec,nosuid,nodev,size=32m',tag],'isolated command')
 equal(digest(directory/'run.log'),r['run_sha256'],'run hash');verify_run(read(directory/'run.log'))
 equal(r['artifact_sha256'],re.search(r'^([a-f0-9]{64})  /out/rubixctl$',read(directory/'run.log').decode(),re.M).group(1),'exported artifact identity')
 equal(digest(directory/'source-hashes.json'),r['inventory_sha256'],'inventory hash');inventory=load(directory/'source-hashes.json')
 equal(inventory.get('Dockerfile'),digest(HERE/'Linux.Dockerfile'),'builder')
 current={n for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']}
 for directory_name in ['crates','third_party','tools/upstream']:
  current.update(str(p.relative_to(ROOT)) for p in (ROOT/directory_name).rglob('Cargo.toml') if 'target' not in p.parts)
 current.update(str(p.relative_to(ROOT)) for p in (ROOT/'.cargo').rglob('*') if p.is_file())
 for crate in ['rubixctl','rubix-platform','rubix-supervisor']:current.update(str(p.relative_to(ROOT)) for p in (ROOT/'crates'/crate).rglob('*.rs') if 'target' not in p.parts)
 current.update('tools/parity/fixtures/management-check/'+n for n in ['cases.json','expected.json'])
 current.update('tools/parity/fixtures/prerequisite-preparation/'+n for n in ['linux_fixture.py','fixture-command.rs'])
 equal(sorted(n for n in inventory if relevant(n)),sorted(current),'compiled inventory')
 for name in current:equal(inventory[name],digest(ROOT/name),'compiled source '+name)
 return r
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('directory',type=pathlib.Path);a=p.parse_args();verify(a.directory);print('PASS: actual Linux management command and source identity')
