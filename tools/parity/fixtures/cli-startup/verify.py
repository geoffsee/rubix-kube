#!/usr/bin/env python3
"""Verify startup semantics, exact observations, provenance and cleanup without refreshing fixtures."""
import argparse,hashlib,json,pathlib,math
HERE=pathlib.Path(__file__).resolve().parent

def require(condition,message):
 if not condition:raise ValueError(message)
STREAM_LIMIT=256*1024
JSON_LIMIT=2*1024*1024
FILE_LIMIT=8*1024*1024
CASE_IDS=('version-long', 'version-short', 'version-invalid-bool-env', 'version-invalid-int-env', 'version-invalid-map-env', 'version-directory-file', 'version-before-full', 'version-invalid-explicit-int', 'version-unknown-flag', 'unknown-before-version', 'help-long-name', 'help-invalid-env', 'help-invalid-explicit-int', 'help-unknown-flag', 'short-h', 'duplicate-bool', 'duplicate-positive-negative', 'duplicate-version-alias', 'duplicate-string', 'bool-equals-false', 'bool-equals-true', 'bool-following-false', 'bool-empty-equals', 'string-empty-argv', 'string-empty-equals', 'integer-empty-equals', 'empty-positional', 'delimiter-empty-tail', 'delimiter-flag-is-positional', 'missing-argument', 'invalid-env-overridden-boolean', 'invalid-env-overridden-integer', 'directory-before-string-map-env', 'primitive-env-before-directory', 'empty-full-env', 'empty-config-env', 'empty-config-equals', 'no-version-print', 'no-help')
RUNNER_KEYS={'run.py','driver.py'}
RECEIPT_HASH_KEYS=RUNNER_KEYS|{'Dockerfile','suite','artifact_descriptor'}
DIAGNOSTICS=('build','create','container','container-inspect','container-inventory','container-remove','container-verify-removal','image-inspect','image-inventory','image-remove','image-verify-removal','volume-inspect','volume-inventory','volume-remove','volume-verify-removal')
FIXTURE_KEYS={'README.md','capture.py','observations.json','suite.json','test_startup.py','verify.py'}|{
 'evidence/'+run+'/'+name for run in ('r3','r4') for name in
 (['result.json','runner-result.json']+[label+'.'+stream for label in DIAGNOSTICS for stream in ('stdout','stderr')]+[f'{i:03d}.'+stream for i in range(len(CASE_IDS)) for stream in ('stdout','stderr')])}
def read(path,limit):
 with path.open('rb') as stream:raw=stream.read(limit+1)
 require(len(raw)<=limit,'input exceeds byte limit')
 return raw
def digest(path):return hashlib.sha256(read(path,FILE_LIMIT)).hexdigest()
def strict_json(raw):
 def pairs(items):
  out={}
  for key,value in items:
   require(key not in out,'duplicate JSON key');out[key]=value
  return out
 def invalid(_):raise ValueError('nonfinite JSON')
 def finite(value):
  number=float(value);require(math.isfinite(number),'nonfinite JSON');return number
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=invalid,parse_float=finite)
def load(path):return strict_json(read(path,JSON_LIMIT))
def validate_provenance(provenance):
 equal(set(provenance['fixture_sha256']),FIXTURE_KEYS)
 equal(set(provenance['runner_source_sha256']),RUNNER_KEYS)
 equal(set(provenance['source_sha256']),{'cmd/kubesolo/main.go','internal/config/flags/flags.go','internal/config/loader.go','internal/config/file.go','go.mod','go.sum'})
 equal(set(provenance['kingpin_source_sha256']),{'app.go','flags.go','parser.go','envar.go','global.go'})
def equal(a,b):
 require(type(a) is type(b),'type mismatch')
 if isinstance(a,dict):
  require(a.keys()==b.keys(),'key inventory')
  for key in a:equal(a[key],b[key])
 elif isinstance(a,list):
  require(len(a)==len(b),'list length')
  for x,y in zip(a,b):equal(x,y)
 else:require(a==b,'value mismatch')
def normalize(stderr):
 result=[]
 for line in stderr.splitlines(keepends=True):
  try:value=strict_json(line)
  except json.JSONDecodeError:value=None
  if type(value) is dict:
   # Only zerolog wall-clock time is nondeterministic. All other fields remain.
   value.pop('time',None);result.append(value)
  else:result.append(line)
 return result

def inspect(directory):
 suite=load(HERE/'suite.json');record=load(directory/'result.json');runner=load(directory/'runner-result.json')
 equal(tuple(c['id'] for c in suite['cases']),CASE_IDS)
 equal(tuple(c['id'] for c in record['cases']),CASE_IDS)
 require(len(set(c['id'] for c in record['cases']))==len(CASE_IDS),'duplicate case identity')
 equal(set(runner['source_sha256']),RECEIPT_HASH_KEYS)
 equal(record['status'],'passed');equal(runner['exit_code'],0);equal(runner['errors'],[])
 equal(record['suite_sha256'],digest(HERE/'suite.json'))
 equal(runner['source_sha256']['suite'],digest(HERE/'suite.json'))
 require(len(record['cases'])==len(suite['cases']),'case inventory')
 equal(record['artifact']['source']['revision'],'2ef1c4787989f11f868f81bb84ae2afd4a49a81d')
 equal(record['artifact']['sha256'],'67348b2560d0f831de20ef722c53cc317e385ab7cfbf406633fbbb6a7b73f81e')
 for name in ('container-verify-removal.stdout','image-verify-removal.stdout','volume-verify-removal.stdout'):equal(read(directory/name,STREAM_LIMIT).decode().strip(),'')
 output={}
 for index,(wanted,actual) in enumerate(zip(suite['cases'],record['cases'])):
  equal(actual['id'],wanted['id']);equal(actual['argv'],wanted['argv']);equal(actual['status'],'passed');equal(actual['failures'],[])
  equal(actual['owned_process_group_absent'],True)
  require(not actual.get('timeout') and not actual.get('remaining_artifact_pids'),'process cleanup')
  equal(actual['exit_code'],wanted['expect']['exit_code'])
  streams={}
  for kind in ('stdout','stderr'):
   name=actual[kind+'_file'];equal(name,f'{index:03d}.{kind}')
   path=directory/name;raw=read(path,STREAM_LIMIT);equal(hashlib.sha256(raw).hexdigest(),actual[kind+'_sha256']);streams[kind]=raw.decode()
  streams['combined']=streams['stdout']+streams['stderr']
  for kind,text in streams.items():
   if kind+'_equals' in wanted['expect']:equal(text,wanted['expect'][kind+'_equals'])
   for marker in wanted['expect'].get(kind+'_contains',[]):require(marker in text,'missing independently expected output')
  output[actual['id']]={'exit_code':actual['exit_code'],'stdout':streams['stdout'],'stderr':normalize(streams['stderr'])}
 # Version must return before the deprecated-full warning, not merely exit zero.
 require('deprecated' not in str(output['version-before-full']['stderr']),'full warning before version')
 return output

def verify(directory):
 output=inspect(directory);provenance=load(HERE/'provenance.json');validate_provenance(provenance)
 for name,wanted in provenance['fixture_sha256'].items():equal(digest(HERE/name),wanted)
 equal(digest(HERE/'observations.json'),provenance['observations_sha256'])
 runner=load(directory/'runner-result.json');record=load(directory/'result.json')
 for name,wanted in provenance['runner_source_sha256'].items():equal(runner['source_sha256'][name],wanted)
 equal(record['driver_sha256'],provenance['runner_source_sha256']['driver.py'])
 equal(output,load(HERE/'observations.json'))
 return output

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('directory',type=pathlib.Path);args=p.parse_args();verify(args.directory);print('startup observations match source expectations and reviewed capture')
if __name__=='__main__':main()
