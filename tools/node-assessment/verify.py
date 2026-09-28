#!/usr/bin/env python3
"""Strict node-assessment evidence, independent expected outcomes."""
import hashlib
import json
from pathlib import Path
import re
import sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
HARNESS=('capture.py','verify.py','iptables.py','consumer.py','Capture.Dockerfile')
EXPECTED={name:('None' if name in ['nft','legacy','empty','invalid-utf8'] else 'Some('+kind+')') for name,kind in [('nft',''),('legacy',''),('empty',''),('invalid-utf8',''),('nonzero','Command'),('missing','Command'),('denied','Command'),('overflow','Capture'),('deadline','Deadline'),('cancel','Cancelled'),('held','Capture')]}
def require(value,message):
 if not value:raise ValueError(message)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def pairs(values):
 result={}
 for key,value in values:
  require(key not in result,'duplicate key');result[key]=value
 return result
def reject(_):raise ValueError('noninteger number')
def load(path):
 require(path.stat().st_size<4*1024*1024,'JSON bound')
 return json.loads(path.read_text(),object_pairs_hook=pairs,parse_constant=reject,parse_float=reject)
def run_command(tag,name):
 return ['docker','run','--name',tag+'-'+name,'--init','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=96','--memory=512m','--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=16m','--tmpfs','/usr/sbin:rw,exec,nosuid,nodev,size=1m,uid=65532,gid=65532,mode=0755','--env','RUBIX_NODE_DISPOSABLE=1','--env','PRIVATE_SENTINEL=must-not-reach-probe',tag,'/bin/sh','-c','sha256sum /out/host_preflight /out/iptables_probe /out/assess_host; /out/host_preflight && /out/iptables_probe --ignored --exact disposable_fixed_probe --nocapture && /out/host_preflight --ignored --exact disposable_configured_assessment --nocapture && /usr/local/bin/python3 /consumer.py; status=$?; /usr/local/bin/python3 /namespace_inventory.py; inventory=$?; test "$status" -eq 0 && test "$inventory" -eq 0']
def records(path):
 require(path.stat().st_size<1024*1024,'log bound');lines=path.read_text().splitlines()
 require(lines.count('RUBIX_NODE_PROBE_COMPLETE cases=11 external_sentinel_preserved=true')==1,'completion')
 require(sum(line.startswith('test result: ok. 9 passed; 0 failed;') for line in lines)==1,'policy tests')
 require(sum(line.startswith('test result: ok. 1 passed; 0 failed;') for line in lines)==2,'probe and configured consumer tests')
 require(lines.count('RUBIX_NODE_ASSESSMENT external=true real_probe=true injected_host_facts=true')==1,'configured consumer')
 binaries={}
 for line in lines:
  match=re.fullmatch(r'([a-f0-9]{64})  /out/(host_preflight|iptables_probe|assess_host)',line)
  if match:
   digest_,name=match.groups();require(name not in binaries,'duplicate binary');binaries[name]=digest_
 require(set(binaries)=={'host_preflight','iptables_probe','assess_host'},'binary inventory')
 rows={}
 for line in lines:
  if not line.startswith('RUBIX_NODE_PROBE '):continue
  match=re.fullmatch(r'RUBIX_NODE_PROBE case=([a-z0-9-]+) result=(None|Some\([A-Za-z]+\)) joined=true reaped=(true|false) elapsed_ms=(\d+)',line)
  require(match is not None,'probe record');name,status,reaped,elapsed=match.groups()
  require(name in EXPECTED and name not in rows,'case inventory')
  require(status==EXPECTED[name] and reaped==('false' if name in ['missing','denied'] else 'true'),'probe semantics')
  require(int(elapsed)<5000 and (name!='deadline' or int(elapsed)>=2000),'deadline')
  rows[name]=dict(status=status,reaped=reaped=='true',elapsed_ms=int(elapsed))
 require(set(rows)==set(EXPECTED),'all probe cases')
 expected={f'RUBIX_NODE_CONSUMER case={name} exit={code} probe_absent=true' for name,code in [('help',0),('version',0),('print',0),('blocked',1)]}
 actual=[line for line in lines if line.startswith('RUBIX_NODE_CONSUMER ')]
 require(len(actual)==4 and set(actual)==expected,'consumer boundary')
 ns=[line[len('RUBIX_NAMESPACE '):] for line in lines if line.startswith('RUBIX_NAMESPACE ')]
 require(len(ns)==1,'namespace record');ns=json.loads(ns[0],object_pairs_hook=pairs)
 require(set(ns)=={'init','shell','helper','processes'},'namespace fields')
 ids=[ns[k] for k in ['init','shell','helper']]
 require(all(type(pid) is int and pid>0 for pid in ids) and ids[0]==1 and len(set(ids))==3,'namespace roles')
 require(type(ns['processes']) is list and all(type(pid) is int for pid in ns['processes']) and sorted(ns['processes'])==sorted(ids),'namespace cleanup')
 return rows,binaries
def relevant(path):
 return (path in {'Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/supervisor-process/namespace_inventory.py'} or path.startswith('.cargo/') or path.endswith('/Cargo.toml') or path.startswith('third_party/') or any(path.startswith('crates/'+crate+'/'+part+'/') for crate in ['rubix-kube','rubix-config','rubix-platform','rubix-supervisor'] for part in ['src','tests','examples']) or path in {'tools/node-assessment/'+name for name in HARNESS})
def current_inventory():
 paths=[ROOT/name for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']]
 for directory in ['.cargo','crates','third_party','tools/upstream','tools/supervisor-process','tools/node-assessment']:
  paths += [path for path in (ROOT/directory).rglob('*') if path.is_file() and not any(part in {'evidence','target','__pycache__','.DS_Store'} for part in path.parts)]
 return {str(path.relative_to(ROOT)):digest(path) for path in paths if relevant(str(path.relative_to(ROOT)))}
def ownership(report):
 tag=report.get('tag','');require(re.fullmatch('rubix-node-assessment-[a-f0-9]{32}',tag) is not None,'tag')
 require(report.get('containers')==[tag+'-first',tag+'-repeat'],'owned containers')
 require(set(report.get('runs',{}))=={'first','repeat'},'repeat inventory')
 for name in ['first','repeat']:require(report['runs'][name].get('command')==run_command(tag,name),'isolated command')
def capture(directory):
 report=load(directory/'receipt.json')
 require(type(report.get('schema')) is int and report['schema']==1,'schema')
 require(re.fullmatch('[a-f0-9]{40}',report.get('source_revision','')) is not None and type(report.get('uncommitted_source_snapshot')) is bool,'source metadata')
 ownership(report)
 require(report.get('errors')==[] and report.get('cleanup_errors')==[] and report.get('remaining_containers')==[] and report.get('remaining_images')==[],'cleanup')
 require(re.fullmatch('sha256:[a-f0-9]{64}',report.get('image_id','')) is not None,'image')
 require(report.get('harness_sha256')=={name:digest(HERE/name) for name in HARNESS},'harness')
 require(report.get('helper_sha256')==digest(ROOT/'tools/defaults/capture.py'),'helper')
 require(report.get('source_inventory_sha256')==digest(directory/'source-inventory.json'),'inventory hash')
 inventory=load(directory/'source-inventory.json');require({name:value for name,value in inventory.items() if relevant(name)}==current_inventory(),'current compiled inputs')
 require(set(report.get('runs',{}))=={'first','repeat'},'repeat inventory')
 binaries=[]
 for name in ['first','repeat']:
  run=report['runs'][name];require(set(run)=={'command','raw_sha256','binary_sha256','records'},'run fields')
  require(run['raw_sha256']==digest(directory/(name+'.log')),'raw binding')
  rows,hashes=records(directory/(name+'.log'))
  require(json.dumps(rows,sort_keys=True)==json.dumps(run['records'],sort_keys=True) and hashes==run['binary_sha256'],'parsed binding');binaries.append(hashes)
 require(binaries[0]==binaries[1],'same binaries repeated')
if __name__=='__main__':capture(Path(sys.argv[1]) if len(sys.argv)>1 else HERE/'evidence')
