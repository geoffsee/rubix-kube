"""Verify trusted executable assertions, independent inventories and bounded capture receipts."""
import hashlib
import json
import math
from pathlib import Path
import re
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
FILES = {'build.log','source-hashes.json','receipt.json','run0.log','run1.log'}
FILE_CASES = ['missing_version','no_candidates','exact','exact.xz','exact.zst','exact.gz',
              'glob_broken_symlink','glob_non_utf8_name','not_recursive','entry_limit','byte_limit',
              'non_utf8_version','unsafe_release','permission_denied','fifo_nonblocking','rc_service_exact','injected_ipv6_unsupported_real_ipv4']
PORT_CASES = ['free_all available']
for port in [2379,6443,10443,6060]:
    if port == 6060: PORT_CASES.append('pprof_disabled unprobed')
    PORT_CASES += [f'ipv4_{port} conflict_then_rebind',f'ipv6_{port} conflict_then_rebind']

def require(condition,message):
    if not condition: raise ValueError(message)
def read(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 8*1024*1024,'invalid bounded evidence file')
    return path.read_bytes()
def digest(path): return hashlib.sha256(read(path)).hexdigest()
def loads(raw):
    def pairs(values):
        result={}
        for key,value in values:
            require(key not in result,'duplicate JSON key');result[key]=value
        return result
    def invalid(_): raise ValueError('nonfinite JSON')
    def finite(value):
        result=float(value);require(math.isfinite(result),'nonfinite JSON');return result
    return json.loads(raw,object_pairs_hook=pairs,parse_constant=invalid,parse_float=finite)
def markers(raw):
    lines = raw.decode().splitlines()
    require([line for line in lines if line.startswith('RUBIX_PREFLIGHT_FILES ')] ==
            ['RUBIX_PREFLIGHT_FILES '+name+' pass' for name in FILE_CASES], 'filesystem case assertions')
    require([line for line in lines if line.startswith('RUBIX_PREFLIGHT_PORTS ')] ==
            ['RUBIX_PREFLIGHT_PORTS '+name for name in PORT_CASES], 'port case assertions')
    require(sum(line.startswith('test result: ok.') for line in lines)==4,'all four test invocations complete')
    require(not any(line.startswith('test result: FAILED') for line in lines),'failed test invocation')
    binaries = [line for line in lines if re.fullmatch(r'[0-9a-f]{64}  /out/(rubix_platform|preflight_probe)-[0-9a-f]+',line)]
    require(len(binaries)==2,'executable inventory')
    return binaries

def verify(here=HERE,root=ROOT):
    evidence=here/'evidence';require({p.name for p in evidence.iterdir()}==FILES,'exact evidence inventory')
    provenance=loads(read(here/'provenance.json'))
    require(type(provenance) is dict and set(provenance)=={'files'} and set(provenance['files'])==FILES,'provenance inventory')
    for name,value in provenance['files'].items(): require(digest(evidence/name)==value,'evidence digest '+name)
    report=loads(read(evidence/'receipt.json'))
    require(type(report) is dict and set(report)=={'schema_version','revision','platform','uncommitted_implementation','containers','errors','cleanup_errors','remaining_containers','remaining_images','source_sha256','runs','image_id','helper_sha256','inventory_sha256'},'receipt inventory')
    require(isinstance(report.get('image_id'),str) and re.fullmatch('sha256:[0-9a-f]{64}',report['image_id']),'image identity')
    require(type(report.get('schema_version')) is int and report['schema_version']==1,'schema')
    require(report.get('platform')=='linux/arm64' and type(report.get('uncommitted_implementation')) is bool,'qualification platform')
    require(isinstance(report.get('revision'),str) and re.fullmatch('[0-9a-f]{40}',report['revision']),'revision')
    for key in ['errors','cleanup_errors','remaining_containers','remaining_images']: require(report.get(key)==[],'cleanup '+key)
    require(isinstance(report.get('containers'),list) and len(report['containers'])==2 and len(set(report['containers']))==2,'owned containers')
    require(all(isinstance(name,str) and re.fullmatch(r'rubix-preflight-probes-[0-9a-f]{32}-[01]',name) for name in report['containers']),'owned container identity')
    require(report['containers'][0][:-1]==report['containers'][1][:-1] and report['containers'][0].endswith('0') and report['containers'][1].endswith('1'),'repeated container identity')
    require(set(report['source_sha256'])=={'capture.py','verify.py','Capture.Dockerfile'},'harness inventory')
    for name,value in report['source_sha256'].items():require(digest(here/name)==value,'current harness '+name)
    require(report['helper_sha256']==digest(root/'tools/defaults/capture.py'),'capture helper')
    require(report['inventory_sha256']==digest(evidence/'source-hashes.json'),'compiled source inventory hash')
    inventory=loads(read(evidence/'source-hashes.json'))
    current={'Cargo.toml','Cargo.lock','rust-toolchain.toml'}
    current.update(str(p.relative_to(root)) for p in (root/'.cargo').rglob('*') if p.is_file())
    for name in ['crates','third_party','tools/upstream']:
        current.update(str(p.relative_to(root)) for p in (root/name).rglob('Cargo.toml') if 'target' not in p.parts)
    current.update(str(p.relative_to(root)) for p in (root/'crates/rubix-platform').rglob('*.rs'))
    bound={name for name in inventory if name.endswith('Cargo.toml') or name in {'Cargo.lock','rust-toolchain.toml'} or name.startswith('.cargo/') or (name.startswith('crates/rubix-platform/') and name.endswith('.rs'))}
    require(current==bound,'compiled input inventory')
    for name in bound:require(inventory[name]==digest(root/name),'compiled source changed '+name)
    for name in ['platform-discovery','preflight-policy']:
        path='tools/parity/fixtures/'+name+'/expected.tsv'
        require(inventory.get(path)==digest(root/path),'compile fixture changed')
    require(inventory.get('Dockerfile')==digest(here/'Capture.Dockerfile'),'build recipe')
    require(set(report['runs'])=={'run0.log','run1.log'},'repeat run inventory')
    binaries=[]
    for name,value in report['runs'].items():
        require(digest(evidence/name)==value,'run digest');binaries.append(markers(read(evidence/name)))
    require(binaries[0]==binaries[1],'same executable repeats')
    return report
if __name__=='__main__':verify();print('Preflight probe evidence verified')
