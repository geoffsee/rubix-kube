#!/usr/bin/env python3
"""Strict verification of bounded, real process ownership evidence."""
import hashlib
import json
import math
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LIMIT = 1024 * 1024
CASES = ['delayed', 'family', 'ignore', 'term-error', 'early', 'leader-exits-first', 'oneshot', 'probe-error', 'worker-panic', 'cancelled', 'spawn-error', 'startup-timeout', 'sentinel-survived-external-stop']

def require(condition, message):
    if not condition: raise ValueError(message)

def bounded(path, limit=LIMIT):
    with Path(path).open('rb') as stream: data = stream.read(limit+1)
    require(len(data) <= limit, 'input exceeds byte budget')
    return data

def strict(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def floating(value):
        result = float(value)
        require(math.isfinite(result), 'nonfinite JSON number')
        return result
    def constant(_value): raise ValueError('nonfinite JSON constant')
    require(len(data) <= 2*LIMIT, 'JSON exceeds byte budget')
    return json.loads(data, object_pairs_hook=pairs, parse_float=floating, parse_constant=constant)

def load(path): return strict(bounded(path,2*LIMIT))
def digest(path, limit=16*LIMIT): return hashlib.sha256(bounded(path,limit)).hexdigest()
def equal(a,b,label): require(json.dumps(a,sort_keys=True,allow_nan=False)==json.dumps(b,sort_keys=True,allow_nan=False), label+' differs')
def keys(value,names,label): require(type(value) is dict and set(value)==set(names),label+' inventory differs')

def normalize(records):
    return [{k:v for k,v in record.items() if k != 'elapsed_ms'} for record in records]

def verify_records(records):
    require(type(records) is list and len(records)==len(CASES),'incomplete cases')
    for record,name in zip(records,CASES,strict=True):
        keys(record, {'case','spawned','term_attempted','kill_attempted','leader_reaped','thread_joined','complete','exit_code','signal','elapsed_ms','error'},'record')
        for key in ['spawned','kill_attempted','leader_reaped','thread_joined','complete']:
            equal(record[key],name != 'spawn-error' or key == 'thread_joined','ownership '+key)
        equal(record['case'],name,'case identity')
        require(type(record['elapsed_ms']) is int and 0 <= record['elapsed_ms'] <= 37000,'elapsed bound')
        graceful = name in ['delayed','family','term-error','startup-timeout','sentinel-survived-external-stop']
        equal(record['term_attempted'],graceful or name=='ignore','TERM expectation')
        code = 17 if name in ['early','leader-exits-first','term-error'] else 0 if graceful or name=='oneshot' else None
        equal(record['exit_code'],code,'exit status')
        equal(record['signal'],None if code is not None or name=='spawn-error' else 9,'termination signal')
        equal(record['error'],'process_spawn_failed' if name=='spawn-error' else None,'adapter error')
        if name=='ignore': require(record['elapsed_ms'] >= 29000,'escalation occurred before real grace')
        elif name=='startup-timeout': require(1000 <= record['elapsed_ms'] <= 5000,'startup timeout bound')
        elif not graceful: equal(record['elapsed_ms'],0,'untimed case')

def raw_records(path):
    raw = bounded(path)
    records = [strict(line[len(b'RUBIX_PROCESS '):]) for line in raw.splitlines() if line.startswith(b'RUBIX_PROCESS ')]
    verify_records(records)
    binaries = re.findall(rb'^([a-f0-9]{64})  /process-tests$',raw,re.M)
    require(len(binaries)==1,'missing/duplicate binary identity')
    inventories = [strict(line[len(b'RUBIX_NAMESPACE '):]) for line in raw.splitlines() if line.startswith(b'RUBIX_NAMESPACE ')]
    require(len(inventories)==1,'missing/duplicate namespace inventory')
    inventory=inventories[0]
    keys(inventory,{'init','shell','helper','processes'},'namespace')
    equal(inventory['init'],1,'namespace init')
    for key in ['shell','helper']:
        require(type(inventory[key]) is int and inventory[key] > 1,'namespace process identity')
    require(type(inventory['processes']) is list and all(type(pid) is int for pid in inventory['processes']),'namespace PID types')
    equal(sorted(inventory['processes']),sorted({1,inventory['shell'],inventory['helper']}),'remaining namespace processes')
    require(len(inventory['processes'])==3,'namespace process count')
    return records,binaries[0].decode()

def source_inventory():
    files = [ROOT/name for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']]
    for name in ['crates','third_party','tools/upstream','tools/supervisor-process']:
        files.extend(path for path in (ROOT/name).rglob('*') if path.is_file() and
                     not set(path.relative_to(ROOT/name).parts).intersection({'target','__pycache__','evidence','.DS_Store'}))
    return {str(path.relative_to(ROOT)):digest(path) for path in sorted(files)}

def relevant_source(name):
    """Inputs used by the supervisor executable or its build/capture/runtime harness."""
    return (name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/rubix-supervisor/Cargo.toml'} or
            name.startswith(('crates/rubix-supervisor/src/','crates/rubix-supervisor/tests/')) or
            name in {'tools/supervisor-process/'+file for file in
                     ['capture.py','verify.py','fixture.py','namespace_inventory.py','Capture.Dockerfile']})

def verify_sources(receipt,directory,current):
    historical=load(directory/'source-inventory.json')
    require(type(historical) is dict and bool(historical),'historical source inventory')
    for name,value in historical.items():
        require(type(name) is str and not Path(name).is_absolute() and
                all(part not in {'..','.'} for part in Path(name).parts),'historical source path')
        require(name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml'} or
                name.startswith(('crates/','third_party/','tools/upstream/','tools/supervisor-process/')),
                'historical source scope')
        require(type(value) is str and re.fullmatch('[a-f0-9]{64}',value),'historical source digest')
    equal(receipt['source_inventory_sha256'],digest(directory/'source-inventory.json'),'historical inventory digest')
    equal(receipt['source_sha256'],historical,'complete historical inventory')
    actual=source_inventory()
    equal({k:v for k,v in historical.items() if relevant_source(k)},
          {k:v for k,v in actual.items() if relevant_source(k)},'current relevant source inventory')
    if current: equal(historical,actual,'complete current source inventory')

def verify_capture(directory, *, current=True):
    directory=Path(directory)
    receipt=load(directory/'receipt.json')
    keys(receipt,{'schema','source_revision','uncommitted_source_snapshot','source_sha256','source_inventory_sha256','driver_sha256','verifier_sha256','runs','cleanup_errors','remaining_containers','remaining_images','build_command','build_exit_code','image_id','repeat_equal'},'receipt')
    equal(receipt['schema'],1,'schema')
    require(type(receipt['source_revision']) is str and re.fullmatch('[a-f0-9]{40}',receipt['source_revision']),'source revision')
    require(receipt['uncommitted_source_snapshot'] is True,'source qualification')
    equal(receipt['build_exit_code'],0,'build exit')
    require(re.fullmatch('sha256:[a-f0-9]{64}',receipt['image_id']) is not None,'image identity')
    equal(receipt['driver_sha256'],digest(HERE/'capture.py'),'current driver')
    equal(receipt['verifier_sha256'],digest(HERE/'verify.py'),'current verifier')
    verify_sources(receipt,directory,current)
    build = receipt['build_command']
    require(type(build) is list and len(build)==8 and all(type(v) is str for v in build),'build command')
    equal(build[:4],['docker','build','--platform=linux/arm64','--tag'],'build command prefix')
    require(re.fullmatch('rubix-process-[a-f0-9]{32}',build[4]) is not None,'owned image name')
    equal(build[5],'--file','build file flag')
    equal(build[6],str(Path(build[7])/'tools/supervisor-process/Capture.Dockerfile'),'build source path')
    require(Path(build[7]).is_absolute(),'build context path')
    keys(receipt['runs'],{'first','repeat'},'runs')
    normalized=[]; binaries=[]
    for name in ['first','repeat']:
        run=receipt['runs'][name]
        keys(run,{'exit_code','raw_sha256','command','binary_sha256'},'run')
        equal(run['exit_code'],0,'run exit')
        equal(run['raw_sha256'],digest(directory/(name+'.log')),'raw digest')
        records,binary=raw_records(directory/(name+'.log'))
        equal(load(directory/(name+'.json')),records,'raw records')
        equal(run['binary_sha256'],binary,'binary digest')
        expected_command = ['docker','run','--name',build[4]+'-'+name,'--init','--network=none','--read-only',
            '--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=96','--memory=512m',
            '--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=64m','--env','RUBIX_PROCESS_DISPOSABLE=1',build[4],
            '/bin/sh','-c','sha256sum /process-tests; /process-tests --ignored --exact disposable_process_cases --nocapture; status=$?; /usr/local/bin/python3 /namespace_inventory.py || exit $?; exit "$status"']
        equal(run['command'],expected_command,'contained run command')
        normalized.append(normalize(records));binaries.append(binary)
    equal(normalized[0],normalized[1],'repeated semantics')
    equal(binaries[0],binaries[1],'repeated binary')
    require(receipt['repeat_equal'] is True,'repeat incomplete')
    for key in ['cleanup_errors','remaining_containers','remaining_images']: equal(receipt[key],[],key)
    return receipt

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path)
    parser.add_argument('--relevant-current',action='store_true',help='Verify complete historical provenance and all current supervisor/harness inputs; allow unrelated source changes')
    args=parser.parse_args()
    verify_capture(args.capture,current=not args.relevant_current)
    print('PASS: bounded real process cases, source/binary identity, independent assertions and cleanup')
