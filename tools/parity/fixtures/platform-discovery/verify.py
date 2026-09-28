"""Strict verification of trusted, frozen platform discovery evidence."""
import hashlib,json,math,pathlib,re
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
SOURCE='ed6900c2832745617930fdcdf77969a63a87a39b15fe8a318805ba077b84648a  detect.go\n'

def require(ok,message):
 if not ok:raise ValueError(message)
def pairs(items):
 result={}
 for key,value in items:
  require(key not in result,'duplicate JSON key');result[key]=value
 return result
def finite(value):
 result=float(value);require(math.isfinite(result),'nonfinite number');return result
def loads(value):
 def invalid(_):raise ValueError('nonfinite constant')
 return json.loads(value,object_pairs_hook=pairs,parse_float=finite,parse_constant=invalid)
def read(path):
 require(path.is_file() and not path.is_symlink() and path.stat().st_size<=8*1024*1024,'invalid evidence file');return path.read_bytes()
def digest(path):return hashlib.sha256(read(path)).hexdigest()
def records(data):
 return [loads(line.removeprefix('RUBIX_CAPTURE ')) for line in data.decode().splitlines() if line.startswith('RUBIX_CAPTURE ')]
def clean(report):
 for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:require(type(report.get(key)) is list and not report[key],'missing or unsuccessful cleanup: '+key)
 require(type(report.get('containers')) is list and len(report['containers'])>0,'missing ownership inventory')
def verify(here=HERE):
 provenance=loads(read(here/'provenance.json'))
 expectedfiles={'go/'+n for n in ['build.log','run0.log','run1.log','receipt.json','source.sha256']}|{'linux/'+n for n in ['build.log','run.log','receipt.json','source-hashes.json']}
 require(set(provenance)=={'files'} and set(provenance['files'])==expectedfiles,'exact evidence inventory')
 actual={str(p.relative_to(here/'evidence')) for p in (here/'evidence').rglob('*') if p.is_file()}
 require(actual==expectedfiles,'unexpected evidence file')
 for name,value in provenance['files'].items():require(digest(here/'evidence'/name)==value,'evidence digest: '+name)
 expected=(here/'expected.tsv').read_text().splitlines()
 go=loads(read(here/'evidence/go/receipt.json'));clean(go)
 require(set(go)=={'revision','archive_sha256','source_sha256','helper_sha256','containers','errors','cleanup_errors','outputs','image_id','identical_records','remaining_containers','remaining_images'},'Go receipt keys')
 require(go['revision']=='2ef1c4787989f11f868f81bb84ae2afd4a49a81d' and go['archive_sha256']=='9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','source authority')
 require(go['identical_records'] is True and len(go['containers'])==2,'Go repeats')
 require(set(go['source_sha256'])=={'capture.py','Capture.Dockerfile','detect_capture_test.go','expected.tsv'},'Go harness inventory')
 require(set(go['outputs'])=={'run0.log','run1.log','source.sha256'},'Go output inventory')
 for name,value in go['source_sha256'].items():require(digest(here/name)==value,'current Go harness: '+name)
 for name,value in go['outputs'].items():require(digest(here/'evidence/go'/name)==value,'Go output digest')
 for name in ['run0.log','run1.log']:require(records(read(here/'evidence/go'/name))==[expected],'exact independent Go records')
 require(read(here/'evidence/go/source.sha256').decode()==SOURCE,'unaltered actual Go source')
 linux=loads(read(here/'evidence/linux/receipt.json'));clean(linux)
 require(set(linux)=={'containers','errors','cleanup_errors','source_sha256','helper_sha256','inventory_sha256','run_sha256','remaining_containers','remaining_images','revision','uncommitted_implementation','platform','image_id'},'Linux receipt keys')
 require(isinstance(linux['revision'],str) and re.fullmatch('[0-9a-f]{40}',linux['revision']) and linux['uncommitted_implementation'] is True and linux['platform']=='linux/arm64','tested source identity')
 require(isinstance(linux['image_id'],str) and re.fullmatch('sha256:[0-9a-f]{64}',linux['image_id']),'Linux image identity')
 require(len(linux['containers'])==1 and set(linux['source_sha256'])=={'qualify.py','capture.py','Linux.Dockerfile','expected.tsv'},'Linux ownership/harness inventory')
 for report in [go,linux]:require(report['helper_sha256']==digest(ROOT/'tools/defaults/capture.py'),'lifecycle helper')
 for name,value in linux['source_sha256'].items():require(digest(here/name)==value,'current Linux harness')
 require(digest(here/'evidence/linux/source-hashes.json')==linux['inventory_sha256'],'build inventory digest')
 inventory=loads(read(here/'evidence/linux/source-hashes.json'))
 required={'Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml','crates/rubix-platform/Cargo.toml','crates/rubix-platform/src/lib.rs','crates/rubix-platform/src/discover.rs','crates/rubix-platform/src/classify.rs','crates/rubix-platform/src/model.rs','crates/rubix-platform/tests/discovery.rs','Dockerfile'}
 require(required<=set(inventory),'missing compiled input')
 current={'Cargo.toml','Cargo.lock','rust-toolchain.toml'}
 current.update(str(p.relative_to(ROOT)) for p in (ROOT/'.cargo').rglob('*') if p.is_file())
 for directory in ['crates','third_party','tools/upstream']:
  current.update(str(p.relative_to(ROOT)) for p in (ROOT/directory).rglob('Cargo.toml') if 'target' not in p.parts)
 current.update(str(p.relative_to(ROOT)) for p in (ROOT/'crates/rubix-platform').rglob('*.rs') if 'target' not in p.parts)
 bound={name for name in inventory if name.endswith('Cargo.toml') or name in {'Cargo.lock','rust-toolchain.toml'} or name.startswith('.cargo/') or (name.startswith('crates/rubix-platform/') and name.endswith('.rs'))}
 require(current==bound,'compiled input inventory changed')
 for name,value in inventory.items():
  if name.endswith('Cargo.toml') or name in {'Cargo.lock','rust-toolchain.toml'} or name.startswith('.cargo/') or (name.startswith('crates/rubix-platform/') and name.endswith('.rs')):require(digest(ROOT/name)==value,'compiled input changed: '+name)
 require(inventory.get('tools/parity/fixtures/preflight-policy/expected.tsv')==digest(ROOT/'tools/parity/fixtures/preflight-policy/expected.tsv'),'compiled preflight fixture changed')
 require(inventory['Dockerfile']==digest(here/'Linux.Dockerfile'),'Linux builder recipe')
 log=read(here/'evidence/linux/run.log');require(hashlib.sha256(log).hexdigest()==linux['run_sha256'],'Linux raw log')
 text=log.decode();require(text.count('test result: ok. 1 passed;')==1 and text.count('test result: ok. 8 passed;')==1 and text.count('test result: ok. 6 passed;')==1 and 'real_discovery_preserves_custom_paths_and_does_not_create_or_rewrite_them ... ok' in text,'Linux completion')
 require(len(re.findall(r'^[0-9a-f]{64}  /out/(?:discovery|rubix_platform|preflight)-[0-9a-f]+$',text,re.MULTILINE))==3,'binary identities')
if __name__=='__main__':verify();print('Platform evidence verified')
