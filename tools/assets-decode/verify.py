"""Strict native decoder evidence; source-bound fixtures, never production payload execution."""
import hashlib,json,os,re,stat,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
HARNESS=('capture.py','verify.py','test_evidence.py','Capture.Dockerfile')
EXPECTED={
'aggregate_budget_is_checked_before_observer_and_retained_on_header_failure',
'all_truncations_corrupt_checksum_and_trailing_members_fail_after_rehash',
'empty_frames_still_require_completion_and_consume_one_probe',
'exact_limit_succeeds_but_excess_and_failed_attempts_retain_budget_charges',
'gzip_header_fields_are_bounded_before_decoder_allocation',
'gzip_optional_fields_and_header_crc_are_checked_without_using_names_as_paths',
'image_limit_and_executable_limit_are_distinct_and_receipt_requires_full_trailer',
'independent_fixed_vectors_match_decoded_hash_and_binding',
'invalid_limits_fail_without_a_session',
'observer_failure_is_typed_provisional_and_display_does_not_leak',
'same_encoded_slice_must_match_before_any_callback',
'unknown_size_rle_bomb_is_streamed_in_fixed_chunks_and_stops_at_limit',
'unknown_zstd_content_size_is_still_bounded_and_checksum_is_checked',
'zstd_skippable_dictionary_large_window_and_reserved_headers_are_rejected'}
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
    return ['docker','run','--name',tag+'-'+name,'--init','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=64','--memory=256m','--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag,'/bin/sh','-c','sha256sum /decode-tests; /decode-tests --nocapture --test-threads=1; status=$?; /usr/local/bin/python3 /namespace_inventory.py; inventory=$?; test "$status" -eq 0 && test "$inventory" -eq 0']
def build_binary(path):
    # --progress=plain and a per-capture nonce make the builder checksum observable.
    # A Dockerfile RUN command echo is not a checksum output record.
    matches=[]
    for line in read(path,16*1024*1024).decode().splitlines():
        match=re.fullmatch(r'#[0-9]+ [0-9]+(?:\.[0-9]+)? ([a-f0-9]{64})  /out/decode-tests',line)
        if match:matches.append(match[1])
    require(len(matches)==1,'unique builder binary digest')
    return matches[0]
def records(path):
    lines=read(path,1024*1024).decode().splitlines()
    require(sum(line.startswith('test result: ok. 14 passed; 0 failed; 0 ignored;') for line in lines)==1,'test completion')
    cases=[]
    for line in lines:
        if line.startswith('test ') and not line.startswith('test result:'):
            match=re.fullmatch(r'test ([a-z0-9_]+) \.\.\. ok',line);require(match is not None,'test outcome');cases.append(match[1])
    require(len(cases)==len(EXPECTED) and set(cases)==EXPECTED,'exact test inventory')
    binaries=[line.split()[0] for line in lines if line.endswith('  /decode-tests')]
    require(len(binaries)==1 and re.fullmatch('[0-9a-f]{64}',binaries[0]) is not None,'binary identity')
    ns=[strict(line.removeprefix('RUBIX_NAMESPACE ')) for line in lines if line.startswith('RUBIX_NAMESPACE ')]
    require(len(ns)==1 and set(ns[0])=={'init','shell','helper','processes'},'namespace schema');ns=ns[0]
    identities=[ns[key] for key in ['init','shell','helper']]
    require(all(type(pid) is int and pid>0 for pid in identities) and identities[0]==1 and len(set(identities))==3,'namespace identity')
    require(type(ns['processes']) is list and all(type(pid) is int for pid in ns['processes']) and sorted(ns['processes'])==sorted(identities),'namespace cleanup')
    return sorted(cases),binaries[0]
def relevant(name):
    return name in {'Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/defaults/capture.py','tools/supervisor-process/namespace_inventory.py'} or name.endswith('/Cargo.toml') or name.startswith('.cargo/') or name.startswith(('crates/rubix-assets/','crates/rubix-platform/')) and name.endswith(('.rs','.json','.bin')) or name in {'tools/assets-decode/'+value for value in HARNESS}
def current_inventory():
    paths=[ROOT/name for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','tools/defaults/capture.py','tools/supervisor-process/namespace_inventory.py']]
    for directory in ['.cargo','crates','third_party','tools/upstream','tools/assets-decode']:
        paths.extend(p for p in (ROOT/directory).rglob('*') if p.is_file() and not any(v in p.parts for v in ['target','evidence','__pycache__','.DS_Store']) and relevant(str(p.relative_to(ROOT))))
    return {str(p.relative_to(ROOT)):digest(p) for p in sorted(set(paths))}
def capture(directory):
    directory=Path(directory);report=load(directory/'receipt.json')
    require(set(report)=={'schema','source_revision','uncommitted_source_snapshot','tag','harness_sha256','helper_sha256','containers','errors','cleanup_errors','runs','source_inventory_sha256','image_id','remaining_containers','remaining_images','build_log_sha256','build_binary_sha256'},'receipt schema')
    require(type(report['schema']) is int and report['schema']==1,'schema');require(type(report['uncommitted_source_snapshot']) is bool,'dirty scope')
    require(re.fullmatch('[a-f0-9]{40}',report['source_revision']) is not None,'revision');tag=report['tag'];require(re.fullmatch('rubix-decode-[a-f0-9]{32}',tag) is not None,'owned tag')
    require(report['containers']==[tag+'-first',tag+'-repeat'],'owned containers')
    for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:require(report[key]==[],key)
    require(re.fullmatch('sha256:[a-f0-9]{64}',report['image_id']) is not None,'image')
    require(report['harness_sha256']=={name:digest(HERE/name) for name in HARNESS},'harness binding');require(report['helper_sha256']==digest(ROOT/'tools/defaults/capture.py'),'helper')
    require(report['source_inventory_sha256']==digest(directory/'source-inventory.json'),'inventory digest');inventory=load(directory/'source-inventory.json')
    require({key:value for key,value in inventory.items() if relevant(key)}==current_inventory(),'exact compiled input inventory')
    require(report['build_log_sha256']==digest(directory/'build.log'),'build log binding')
    builder=build_binary(directory/'build.log')
    require(report['build_binary_sha256']==builder,'builder observation binding')
    require(set(report['runs'])=={'first','repeat'},'two runs');binaries=[]
    for name,run in report['runs'].items():
        require(set(run)=={'command','raw_sha256','binary_sha256','records'},'run schema');require(run['command']==run_command(tag,name),'isolated command')
        require(run['raw_sha256']==digest(directory/(name+'.log')),'raw log binding');cases,binary=records(directory/(name+'.log'))
        require(run['records']==cases and run['binary_sha256']==binary,'raw observation binding')
        require(binary==builder,'builder/runtime binary mismatch');binaries.append(binary)
    require(binaries[0]==binaries[1],'repeated executable')
if __name__=='__main__':capture(Path(sys.argv[1]) if len(sys.argv)>1 else HERE/'evidence');print('Native Linux decoder qualification verified')
