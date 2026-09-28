"""Strict source-bound independent expectation verification; trusted repository inputs."""
import hashlib,json,math,pathlib,re
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
REVISION='2ef1c4787989f11f868f81bb84ae2afd4a49a81d'
ARCHIVE='9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec'
FAMILIES=['network','system','proxy','embedded']
BUILD_INPUTS=['Capture.Dockerfile','extract.go','go.mod']+[f+'_test.go' for f in FAMILIES]
INPUTS=BUILD_INPUTS+['capture.py','verify.py','expected.json','source-pins.json']
def require(ok,message):
 if not ok:raise ValueError(message)
def strict(raw):
 def pairs(items):
  value={}
  for k,v in items:
   require(k not in value,'duplicate key');value[k]=v
  return value
 def number(value):
  result=float(value);require(math.isfinite(result),'nonfinite number');return result
 def constant(_):raise ValueError('nonfinite constant')
 return json.loads(raw,object_pairs_hook=pairs,parse_float=number,parse_constant=constant)
def read(path):
 with pathlib.Path(path).open('rb') as stream:raw=stream.read(1048577)
 require(len(raw)<=1048576,'byte budget');return raw
def load(path):return strict(read(path))
def digest(path):return hashlib.sha256(read(path)).hexdigest()
def equal(a,b,message):require(json.dumps(a,sort_keys=True,allow_nan=False)==json.dumps(b,sort_keys=True,allow_nan=False),message)
def records(path):
 rows=[strict(line[14:]) for line in read(path).splitlines() if line.startswith(b'RUBIX_CAPTURE ')]
 require(len(rows)==1,'exact record count');return rows[0]
def verify(directory):
 directory=pathlib.Path(directory);receipt=load(directory/'receipt.json')
 equal(receipt['revision'],REVISION,'revision');equal(receipt['archive_sha256'],ARCHIVE,'archive')
 equal(receipt['source_sha256'],{n:digest(HERE/n) for n in INPUTS},'exact source inventory')
 equal(receipt['helper_sha256'],digest(ROOT/'tools/defaults/capture.py'),'helper')
 for name in ['errors','cleanup_errors','remaining_containers','remaining_images']:equal(receipt[name],[],name)
 require(receipt['identical_records'] is True,'repeat')
 require(re.fullmatch('sha256:[a-f0-9]{64}',receipt['image_id']) is not None,'image identity')
 expected=load(HERE/'expected.json');equal(set_to_list(expected),sorted(FAMILIES),'families')
 outputs={f'{family}{i}.log' for family in FAMILIES for i in range(2)}|{'source.sha256'}
 equal(sorted(receipt['outputs']),sorted(outputs),'output inventory')
 for name in outputs:equal(receipt['outputs'][name],digest(directory/name),'output hash')
 for family in FAMILIES:
  for i in range(2):equal(records(directory/f'{family}{i}.log'),expected[family],family+' observations')
 lines=read(directory/'source.sha256').decode().splitlines()
 source={}
 for line in lines:
  match=re.fullmatch(r'([a-f0-9]{64})  (.+)',line);require(match is not None,'source hash row')
  name=match.group(2);require(name not in source,'duplicate source hash');source[name]=match.group(1)
 pins=load(HERE/'source-pins.json')
 equal(sorted(source),sorted(list(pins)+['/'+family+'.test' for family in FAMILIES]),'source/binary inventory')
 equal({k:source[k] for k in pins},pins,'official baseline source hashes')
 return receipt
def set_to_list(value):
 require(type(value) is dict,'object');return sorted(value)
if __name__=='__main__':
 import argparse
 parser=argparse.ArgumentParser();parser.add_argument('directory',type=pathlib.Path);args=parser.parse_args();verify(args.directory);print('PASS: exact repeated Go observations and cleanup')
