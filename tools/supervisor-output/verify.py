#!/usr/bin/env python3
"""Strict source-bound output evidence; no generated implementation oracle."""
import hashlib
import json
from pathlib import Path
import re
import sys
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
HARNESS = ('capture.py','verify.py','fixture.py','Capture.Dockerfile')
EXPECTED = {'merged':('Complete',8),'empty':('Complete',0),'exact':('Complete',64),
            'overflow':('LimitExceeded',64),'simultaneous':('Complete',2000),
            'flood':('LimitExceeded',64),'missing':('SpawnFailed',0),
            'stop':('Cancelled',None),'timeout':('Cancelled',None),'abort':('Cancelled',None),
            'probe-failure':('Cancelled',None),'descendant':('Complete',6),'escaped':('IncompleteAfterExit',6)}
def require(value, message):
    if not value: raise ValueError(message)
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def pairs(values):
    result = {}
    for key,value in values:
        require(key not in result, 'duplicate JSON key')
        result[key]=value
    return result
def reject(value): raise ValueError('nonfinite JSON')
def load(path):
    require(path.stat().st_size <= 4*1024*1024, 'JSON size')
    return json.loads(path.read_text(), object_pairs_hook=pairs, parse_constant=reject, parse_float=reject)
def run_command(tag, name):
    return ['docker','run','--name',tag+'-'+name,'--init','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=96','--memory=512m','--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=16m','--env','RUBIX_OUTPUT_DISPOSABLE=1',tag,'/bin/sh','-c','sha256sum /output-tests; /output-tests --ignored --exact disposable_output_cases --nocapture; status=$?; /usr/local/bin/python3 /namespace_inventory.py; inventory=$?; test "$status" -eq 0 && test "$inventory" -eq 0']
def ownership(report):
    tag=report.get('tag','')
    require(re.fullmatch('rubix-output-[0-9a-f]{32}',tag) is not None,'owned tag')
    require(report.get('containers')==[tag+'-first',tag+'-repeat'],'owned containers')
    require(set(report.get('runs',{}))=={'first','repeat'},'repeat inventory')
    for name in ('first','repeat'):
        require(report['runs'][name].get('command')==run_command(tag,name),'reviewed command')
def records(path):
    require(path.stat().st_size <= 1024*1024, 'log size')
    lines=path.read_text().splitlines()
    require(lines.count('RUBIX_OUTPUT_COMPLETE cases=13') == 1, 'completion')
    require(sum(line.startswith('test result: ok. 1 passed; 0 failed;') for line in lines)==1,'test result')
    binaries=[line.split()[0] for line in lines if line.endswith('  /output-tests')]
    require(len(binaries)==1 and re.fullmatch('[0-9a-f]{64}',binaries[0]),'binary hash')
    result={}
    pattern=r'RUBIX_OUTPUT case=([a-z-]+) status=([A-Za-z]+) bytes=(\d+) reaped=(true|false) joined=(true|false) elapsed_ms=(\d+)'
    for line in lines:
        if not line.startswith('RUBIX_OUTPUT '): continue
        match=re.fullmatch(pattern,line)
        require(match is not None,'record shape')
        case,status,size,reaped,joined,elapsed=match.groups()
        require(case in EXPECTED and case not in result,'case identity')
        expected_status,expected_size=EXPECTED[case]
        require(status==expected_status and (expected_size is None or int(size)==expected_size),'capture semantics')
        require(int(size)<= (2000 if case=='simultaneous' else 64),'byte budget')
        require(joined=='true' and reaped==('false' if case=='missing' else 'true'),'owner retirement')
        require(int(elapsed)<5000,'case deadline')
        result[case]=dict(status=status,bytes=int(size),reaped=reaped=='true',joined=True,elapsed_ms=int(elapsed))
    require(set(result)==set(EXPECTED),'case inventory')
    namespace=[line.removeprefix('RUBIX_NAMESPACE ') for line in lines if line.startswith('RUBIX_NAMESPACE ')]
    require(len(namespace)==1,'namespace inventory')
    ns=json.loads(namespace[0],object_pairs_hook=pairs,parse_constant=reject,parse_float=reject)
    require(set(ns)=={'init','shell','helper','processes'},'namespace fields')
    require(all(type(ns[key]) is int and ns[key]>0 for key in ('init','shell','helper')) and ns['init']==1 and ns['shell']>1 and ns['helper']>1 and len({ns['init'],ns['shell'],ns['helper']})==3,'namespace identities')
    require(type(ns['processes']) is list and all(type(pid) is int for pid in ns['processes']) and sorted(ns['processes'])==sorted([ns['init'],ns['shell'],ns['helper']]),'namespace cleanup')
    return result,binaries[0]
def relevant(path):
    return (path in {'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/rubix-supervisor/Cargo.toml','tools/supervisor-process/namespace_inventory.py'}
            or path.startswith(('.cargo/','crates/rubix-supervisor/src/','crates/rubix-supervisor/tests/'))
            or path in {'tools/supervisor-output/'+name for name in HARNESS})
def current_inventory():
    paths=[ROOT/name for name in ('Cargo.toml','Cargo.lock','rust-toolchain.toml')]
    for name in ('.cargo','crates/rubix-supervisor','tools/supervisor-output'):
        paths += [path for path in (ROOT/name).rglob('*') if path.is_file() and not any(part in {'evidence','__pycache__','target','.DS_Store'} for part in path.parts)]
    paths.append(ROOT/'tools/supervisor-process/namespace_inventory.py')
    return {str(path.relative_to(ROOT)):digest(path) for path in paths if relevant(str(path.relative_to(ROOT)))}
def capture(directory):
    report=load(directory/'receipt.json')
    require(type(report.get('schema')) is int and report['schema']==1,'schema')
    require(re.fullmatch('[0-9a-f]{40}',report.get('source_revision','')) is not None,'revision')
    require(type(report.get('uncommitted_source_snapshot')) is bool,'snapshot statement')
    ownership(report)
    require(report.get('errors')==[] and report.get('cleanup_errors')==[] and report.get('remaining_containers')==[] and report.get('remaining_images')==[],'cleanup')
    require(re.fullmatch('sha256:[0-9a-f]{64}',report.get('image_id','')) is not None,'image')
    require(report.get('harness_sha256')=={name:digest(HERE/name) for name in HARNESS},'harness')
    require(report.get('helper_sha256')==digest(ROOT/'tools/defaults/capture.py'),'helper')
    require(report.get('source_inventory_sha256')==digest(directory/'source-inventory.json'),'inventory hash')
    inventory=load(directory/'source-inventory.json')
    require({name:value for name,value in inventory.items() if relevant(name)}==current_inventory(),'current relevant input set')
    require(set(report.get('runs',{}))=={'first','repeat'},'repeat inventory')
    binaries=[]
    for name in ('first','repeat'):
        run=report['runs'][name]
        require(set(run)=={'command','raw_sha256','binary_sha256','records'},'run fields')
        require(run['raw_sha256']==digest(directory/(name+'.log')),'raw binding')
        rows,binary=records(directory/(name+'.log'))
        require(json.dumps(rows,sort_keys=True)==json.dumps(run['records'],sort_keys=True),'raw records')
        require(binary==run['binary_sha256'],'binary binding')
        binaries.append(binary)
    require(binaries[0]==binaries[1],'same binary repeated')
if __name__=='__main__': capture(Path(sys.argv[1]) if len(sys.argv)>1 else HERE/'evidence')
