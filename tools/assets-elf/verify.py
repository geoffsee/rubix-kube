"""Verify source-bound, repeated identity ELF observations, not execution compatibility."""
import argparse,json,pathlib,re,stat,os,hashlib
from qualify import ROOT,digest,sources

def require(value,message):
    if not value:raise ValueError(message)
def pairs(values):
    result={}
    for key,value in values:
        require(key not in result,'duplicate JSON key');result[key]=value
    return result
def strict(raw):return json.loads(raw,object_pairs_hook=pairs,parse_float=lambda _:(_ for _ in ()).throw(ValueError('floating JSON not permitted')),parse_constant=lambda _:(_ for _ in ()).throw(ValueError('nonfinite')))
def read(path,limit):
    require(not path.is_symlink(),'evidence symlink')
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NONBLOCK|os.O_NOFOLLOW),'rb') as stream:
        require(stat.S_ISREG(os.fstat(stream.fileno()).st_mode),'regular evidence')
        raw=stream.read(limit+1)
    require(len(raw)<=limit,'evidence size limit')
    return raw
def equal(left,right,message):require(json.dumps(left,sort_keys=True)==json.dumps(right,sort_keys=True),message)
def records(raw,artifacts):
    require(len(raw)<=8*1024*1024,'log limit')
    text=raw.decode();require('test result: ok. 1 passed; 0 failed;' in text,'real test completion')
    rows=[strict(line.removeprefix('RUBIX_ELF ')) for line in text.splitlines() if line.startswith('RUBIX_ELF ')]
    require(len(rows)==4,'four artifact results');seen=set()
    for row in rows:
        require(set(row)=={'role','architecture','bytes','machine','elf_type','entry','flags','interpreter','needed','loader','loader_relation','sha256'},'record schema')
        name=row['role']+'-'+row['architecture'];require(name in artifacts and name not in seen,'artifact inventory');seen.add(name)
        equal(row['sha256'],artifacts[name]['sha256'],'parsed bytes pin');oracle=artifacts[name]['oracle'];equal({key:row[key] for key in oracle},oracle,'independent ELF fields')
        require(row['machine']==(183 if row['architecture']=='arm64' else 62),'actual target machine')
        loader=oracle['interpreter']
        family='Glibc' if loader in ['/lib64/ld-linux-x86-64.so.2','/lib/ld-linux-aarch64.so.1'] else 'Musl' if loader in ['/lib/ld-musl-x86_64.so.1','/lib/ld-musl-aarch64.so.1'] else 'Unknown'
        equal(row['loader'],family,'loader fact');equal(row['loader_relation'],'KnownMismatch' if family=='Musl' else 'Unresolved','no compatibility overclaim')
    return sorted(rows,key=lambda row:(row['role'],row['architecture']))
def verify(directory):
    directory=pathlib.Path(directory);receipt=strict(read(directory/'receipt.json',1024*1024))
    require(set(receipt)=={'revision','working_tree_snapshot','source_sha256','artifacts','errors','qualification','logs'},'receipt schema')
    require(type(receipt['revision']) is str and re.fullmatch('[a-f0-9]{40}',receipt['revision']) is not None,'revision')
    require(receipt['working_tree_snapshot'] is True,'snapshot scope');equal(receipt['errors'],[],'qualification errors');equal(receipt['source_sha256'],sources(),'current source hashes')
    equal(receipt['qualification'],'read-only ELF inspection; no artifact execution','scope')
    inputs=strict(read(ROOT/'experiments/component-boundary/inputs.json',1024*1024))['artifacts']
    expected={f'{role}-{arch}':record for arch,roles in inputs.items() for role,record in roles.items()}
    require(set(receipt['artifacts'])==set(expected),'complete artifact pins')
    for name,record in receipt['artifacts'].items():
        require(set(record)=={'url','sha256','oracle'},'artifact schema')
        equal(record['url'],expected[name]['url'],'artifact URL');equal(record['sha256'],expected[name]['sha256'],'artifact SHA')
        require(set(record['oracle'])=={'bytes','machine','elf_type','entry','flags','interpreter','needed'},'oracle schema')
        require(type(record['oracle']['bytes']) is int and 0<record['oracle']['bytes']<=256*1024*1024,'artifact size')
    require(set(receipt['logs'])=={'run0.log','run1.log'},'two runs')
    results=[]
    for name,value in receipt['logs'].items():
        raw=read(directory/name,8*1024*1024);require(hashlib.sha256(raw).hexdigest()==value,'raw log digest');results.append(records(raw,receipt['artifacts']))
    equal(results[0],results[1],'repeated observation equality')
    return receipt
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('directory',type=pathlib.Path);args=parser.parse_args();verify(args.directory);print('PASS: exact pinned ELF byte/header observations, not execution qualification')
