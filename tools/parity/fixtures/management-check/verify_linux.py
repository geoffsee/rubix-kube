import pathlib,re,hashlib
from verify import HERE,ROOT,require,equal,load,strict,read,digest
HARNESS=['qualify.py','Linux.Dockerfile','linux_fixture.py','verify_linux.py','capture.py']
NAMES=['help','version','root_pass','nonroot','preparation','pprof_off','pprof_conflict','repeat_0','repeat_1']
EXITS=[0,0,0,1,0,0,1,0,0]
def verify_run(raw):
 text=raw.decode();require(text.count('test result: ok. 7 passed;')==1,'Rust test completion')
 binaries=re.findall(r'^([a-f0-9]{64})  /out/(rubixctl|check-tests)$',text,re.M)
 require(len(binaries)==2 and sorted(name for _,name in binaries)==['check-tests','rubixctl'],'binary identity')
 records=[strict(line[12:]) for line in raw.splitlines() if line.startswith(b'RUBIX_CHECK ')]
 require(len(records)==1,'exact executable record count');record=records[0]
 require(type(record) is dict and set(record)=={'cases','listener_survived','ports_released','files_unchanged'},'executable schema')
 for key in ['listener_survived','ports_released','files_unchanged']:require(record[key] is True,key)
 require(type(record['cases']) is list and len(record['cases'])==len(NAMES),'case count')
 for row,name,code in zip(record['cases'],NAMES,EXITS,strict=True):
  require(type(row) is dict and set(row)=={'name','exit','stdout','stderr'},'case schema');equal(row['name'],name,'case name');equal(row['exit'],code,'case exit')
  require(type(row['stdout']) is str and type(row['stderr']) is str,'output type')
  if name in ['help','version']:require('rubixctl' in row['stdout'] and row['stderr']=='','early output')
  else:
   require(row['stdout']=='','check stdout')
   if code==0:require('All 7 checks passed' in row['stderr'] and 'ports are not reserved' in row['stderr'],'qualified success')
   else:require('All 7 checks passed' not in row['stderr'],'false success')
 require('root privileges required' in record['cases'][3]['stderr'],'root failure')
 require('All 7 checks passed' in record['cases'][4]['stderr'],'already prepared host')
 require('TCP port 6060' in record['cases'][6]['stderr'],'pprof conflict')
 return record

def relevant(name):
 return name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml'} or name.endswith('/Cargo.toml') or name.startswith('.cargo/') or name.startswith(('crates/rubix-platform/','crates/rubixctl/','crates/rubix-supervisor/')) and name.endswith('.rs') or name in {'tools/parity/fixtures/management-check/'+n for n in ['cases.json','expected.json','linux_fixture.py']}
def verify(directory):
 directory=pathlib.Path(directory);r=load(directory/'receipt.json')
 require(set(r)=={'revision','uncommitted_implementation','platform','target','containers','errors','cleanup_errors','helper_sha256','source_sha256','inventory_sha256','image_id','command','run_sha256','remaining_containers','remaining_images'},'receipt schema')
 require(type(r['revision']) is str and re.fullmatch('[a-f0-9]{40}',r['revision']) is not None,'revision');require(r['uncommitted_implementation'] is True,'working tree qualification')
 require(type(r['image_id']) is str and re.fullmatch('sha256:[a-f0-9]{64}',r['image_id']) is not None,'image identity')
 equal(r['platform'],'linux/arm64','platform');equal(r['target'],'aarch64-unknown-linux-musl','target')
 for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:equal(r[key],[],key)
 equal(r['source_sha256'],{n:digest(HERE/n) for n in HARNESS},'harness hashes');equal(r['helper_sha256'],digest(ROOT/'tools/defaults/capture.py'),'helper')
 require(type(r['containers']) is list and len(r['containers'])==1 and re.fullmatch('rubix-management-linux-[a-f0-9]{32}-test',r['containers'][0]) is not None,'owned container')
 tag=r['containers'][0][:-5]
 equal(r['command'],['docker','run','--name',tag+'-test','--hostname','fixture','--init','--read-only','--network','none','--cap-drop','ALL','--cap-add','SYS_CHROOT','--cap-add','SETUID','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,exec,nosuid,nodev,size=32m',tag],'isolated command')
 equal(digest(directory/'run.log'),r['run_sha256'],'run hash');verify_run(read(directory/'run.log'))
 equal(digest(directory/'source-hashes.json'),r['inventory_sha256'],'inventory hash');inventory=load(directory/'source-hashes.json')
 equal(inventory.get('Dockerfile'),digest(HERE/'Linux.Dockerfile'),'builder')
 current={n for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']}
 for directory_name in ['crates','third_party','tools/upstream']:
  current.update(str(p.relative_to(ROOT)) for p in (ROOT/directory_name).rglob('Cargo.toml') if 'target' not in p.parts)
 current.update(str(p.relative_to(ROOT)) for p in (ROOT/'.cargo').rglob('*') if p.is_file())
 for crate in ['rubixctl','rubix-platform','rubix-supervisor']:current.update(str(p.relative_to(ROOT)) for p in (ROOT/'crates'/crate).rglob('*.rs') if 'target' not in p.parts)
 current.update('tools/parity/fixtures/management-check/'+n for n in ['cases.json','expected.json','linux_fixture.py'])
 equal(sorted(n for n in inventory if relevant(n)),sorted(current),'compiled inventory')
 for name in current:equal(inventory[name],digest(ROOT/name),'compiled source '+name)
 return r
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('directory',type=pathlib.Path);a=p.parse_args();verify(a.directory);print('PASS: actual Linux management command and source identity')
