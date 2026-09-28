"""Exact source/build/runtime bindings for synthetic pinned-crane serialization evidence."""
import base64,hashlib,json,os,re,stat,sys
from pathlib import Path
import oracle
import runtime
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
HARNESS=('capture.py','verify.py','test_evidence.py','Capture.Dockerfile','main.go','go.mod','go.sum','modules.json','graph.py','oracle.py','runtime.py')
def require(value,message):
    if not value:raise ValueError(message)
def read(path,limit=4*1024*1024):
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NONBLOCK|os.O_NOFOLLOW),'rb') as stream:
        require(stat.S_ISREG(os.fstat(stream.fileno()).st_mode),'regular evidence')
        data=stream.read(limit+1)
    require(len(data)<=limit,'evidence limit');return data
def digest(path):return hashlib.sha256(read(path,16*1024*1024)).hexdigest()
def reject(_):raise ValueError('nonintegral JSON')
def pairs(items):
    value={}
    for key,item in items:require(key not in value,'duplicate key');value[key]=item
    return value
def strict(raw):return json.loads(raw,object_pairs_hook=pairs,parse_float=reject,parse_constant=reject)
def load(path):return strict(read(path))
def run_command(tag,name):
    return ['docker','run','--name',tag+'-'+name,'--init','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=64','--memory=256m','--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag,'/bin/sh','-c',"sha256sum /producer /layer-tests && /usr/local/bin/python3 /runtime.py; runtime_status=$?; /usr/local/bin/python3 /namespace_inventory.py; inventory=$?; test \"$runtime_status\" -eq 0 && test \"$inventory\" -eq 0"]
def build_binary(path):
    matches={}
    for line in read(path,16*1024*1024).decode().splitlines():
        match=re.fullmatch(r'#[0-9]+ [0-9]+(?:\.[0-9]+)? ([a-f0-9]{64})  /out/(producer|layer-tests)',line)
        if match:
            require(match[2] not in matches,'duplicate builder binary digest');matches[match[2]]=match[1]
    require(set(matches)=={'producer','layer-tests'},'complete builder binary inventory')
    return matches
def records(path):
    raw=read(path,2*1024*1024).decode();lines=raw.splitlines()
    binaries={};inputs={};producer=[];complete=[];namespace=[];upstream=[]
    for index,line in enumerate(lines):
        match=re.fullmatch(r'([a-f0-9]{64})  /(producer|layer-tests)',line)
        if match:
            require(index==len(binaries) and match[2] not in binaries,'binary preflight identity/order')
            binaries[match[2]]=match[1]
        elif line.startswith('RUBIX_PRODUCER '):
            require(index==2,'producer ordering');producer.append(line)
        elif line.startswith('RUBIX_UPSTREAM '):
            require(index==3,'upstream oracle ordering');upstream.append(strict(line.removeprefix('RUBIX_UPSTREAM ')))
        elif line.startswith('RUBIX_INPUT '):
            row=strict(line.removeprefix('RUBIX_INPUT '))
            require(set(row)=={'case','gzip_base64'} and row['case'] not in inputs,'input schema/identity')
            require(index==4+len(inputs),'input order')
            inputs[row['case']]=base64.b64decode(row['gzip_base64'],validate=True)
        elif line.startswith('RUBIX_COMPLETE '):complete.append(strict(line.removeprefix('RUBIX_COMPLETE ')))
        elif line.startswith('RUBIX_NAMESPACE '):namespace.append(strict(line.removeprefix('RUBIX_NAMESPACE ')))
        elif line.startswith('RUBIX_'):require(line.startswith('RUBIX_LAYER '),'unknown runtime record')
    require(set(binaries)=={'producer','layer-tests'},'binary inventory')
    require(producer==['RUBIX_PRODUCER go1.26.2 github.com/google/go-containerregistry v0.21.5 h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM='],'exact producer provenance')
    require(len(upstream)==1,'exact upstream oracle');expected=oracle.observations(inputs,upstream[0])
    observed=runtime.parse_consumer(raw)
    require(oracle.same_json(observed,expected),'independent archive observations')
    require(complete==[{'cases':list(oracle.NAMES),'consumer_command':runtime.CONSUMER}],'exact runtime completion')
    require(len(namespace)==1 and set(namespace[0])=={'init','shell','helper','processes'},'namespace schema')
    ns=namespace[0];identities=[ns[key] for key in ['init','shell','helper']]
    require(all(type(pid) is int and pid>0 for pid in identities) and identities[0]==1 and len(set(identities))==3,'namespace identity')
    require(type(ns['processes']) is list and all(type(pid) is int for pid in ns['processes']) and sorted(ns['processes'])==sorted(identities),'namespace cleanup')
    require(lines[-1].startswith('RUBIX_NAMESPACE ') and lines[-2].startswith('RUBIX_COMPLETE '),'final runtime boundaries')
    return expected,binaries
def relevant(name):
    return name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/defaults/capture.py','tools/supervisor-process/namespace_inventory.py'} or name.endswith('/Cargo.toml') or name.startswith('.cargo/') or name.startswith(('crates/rubix-assets/','crates/rubix-platform/')) and name.endswith(('.rs','.json','.bin')) or name in {'tools/assets-layer/'+value for value in HARNESS}
def current_inventory():
    paths=[ROOT/name for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/defaults/capture.py','tools/supervisor-process/namespace_inventory.py']]
    for directory in ['.cargo','crates','third_party','tools/upstream','tools/assets-layer']:
        paths.extend(p for p in (ROOT/directory).rglob('*') if p.is_file() and not any(v in p.parts for v in ['target','evidence','__pycache__','.DS_Store']) and relevant(str(p.relative_to(ROOT))))
    return {str(p.relative_to(ROOT)):digest(p) for p in sorted(set(paths))}
def capture(directory):
    directory=Path(directory);report=load(directory/'receipt.json')
    require(set(report)=={'schema','source_revision','uncommitted_source_snapshot','tag','harness_sha256','helper_sha256','containers','errors','cleanup_errors','runs','source_inventory_sha256','image_id','remaining_containers','remaining_images','build_log_sha256','build_binary_sha256'},'receipt schema')
    require(type(report['schema']) is int and report['schema']==1,'schema');require(report['uncommitted_source_snapshot'] is False,'clean source required')
    require(re.fullmatch('[a-f0-9]{40}',report['source_revision']) is not None,'revision');tag=report['tag'];require(re.fullmatch('rubix-layer-[a-f0-9]{32}',tag) is not None,'owned tag')
    require(report['containers']==[tag+'-first',tag+'-repeat'],'owned containers')
    for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:require(report[key]==[],key)
    require(re.fullmatch('sha256:[a-f0-9]{64}',report['image_id']) is not None,'image')
    require(report['harness_sha256']=={name:digest(HERE/name) for name in HARNESS},'harness binding');require(report['helper_sha256']==digest(ROOT/'tools/defaults/capture.py'),'helper')
    require(report['source_inventory_sha256']==digest(directory/'source-inventory.json'),'inventory digest');inventory=load(directory/'source-inventory.json')
    require({key:value for key,value in inventory.items() if relevant(key)}==current_inventory(),'exact compiled input inventory')
    require(report['build_log_sha256']==digest(directory/'build.log'),'build log binding')
    builder=build_binary(directory/'build.log')
    require(report['build_binary_sha256']==builder,'builder observation binding')
    require(set(report['runs'])=={'first','repeat'},'two runs');binaries=[]; observations=[]
    for name,run in report['runs'].items():
        require(set(run)=={'command','raw_sha256','binary_sha256','records'},'run schema');require(run['command']==run_command(tag,name),'isolated command')
        require(run['raw_sha256']==digest(directory/(name+'.log')),'raw log binding');cases,binary=records(directory/(name+'.log'))
        require(oracle.same_json(run['records'],cases) and run['binary_sha256']==binary,'raw observation binding')
        require(binary==builder,'builder/runtime binary mismatch');binaries.append(binary); observations.append(cases)
    require(binaries[0]==binaries[1],'repeated executables'); require(observations[0]==observations[1],'deterministic repeated archives and observations')
if __name__=='__main__':capture(Path(sys.argv[1]) if len(sys.argv)>1 else HERE/'evidence');print('Pinned synthetic crane layer qualification verified')
