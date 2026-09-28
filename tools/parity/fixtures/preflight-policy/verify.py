"""Validate frozen trusted Go preflight oracle; never run host checks."""
import hashlib,json,math,pathlib,re
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
FILES={'build.log','run0.log','run1.log','receipt.json','source.sha256'}
SOURCE='dd8d85a0593792886b627518b770645e13a9184e5f43a2dd55c071394973cb3e  preflight.go'
HARNESS={'capture.py','Capture.Dockerfile','preflight_capture_test.go','expected.tsv','go.mod'}
def require(value,message):
 if not value:raise ValueError(message)
def pairs(items):
 result={}
 for key,value in items:
  require(key not in result,'duplicate key');result[key]=value
 return result
def finite(value):
 result=float(value);require(math.isfinite(result),'nonfinite number');return result
def loads(value):
 def invalid(_):raise ValueError('nonfinite constant')
 return json.loads(value,object_pairs_hook=pairs,parse_float=finite,parse_constant=invalid)
def read(path):
 require(path.is_file() and not path.is_symlink(),'regular evidence file required')
 with path.open('rb') as stream:data=stream.read(8*1024*1024+1)
 require(len(data)<=8*1024*1024,'file limit');return data
def digest(path):return hashlib.sha256(read(path)).hexdigest()
def records(data):
 return [loads(line.removeprefix('RUBIX_CAPTURE ')) for line in data.decode().splitlines() if line.startswith('RUBIX_CAPTURE ')]
def verify(here=HERE):
 provenance=loads(read(here/'provenance.json'))
 require(type(provenance) is dict and set(provenance)=={'files'},'provenance keys')
 require(type(provenance['files']) is dict and set(provenance['files'])==FILES,'evidence inventory')
 require({p.name for p in (here/'evidence').iterdir()}==FILES,'unexpected evidence entry')
 for name,value in provenance['files'].items():require(digest(here/'evidence'/name)==value,'evidence digest')
 report=loads(read(here/'evidence/receipt.json'))
 require(type(report) is dict and set(report)=={'revision','archive_sha256','source_sha256','helper_sha256','containers','errors','cleanup_errors','outputs','image_id','identical_records','remaining_containers','remaining_images'},'receipt keys')
 require(report['revision']=='2ef1c4787989f11f868f81bb84ae2afd4a49a81d' and report['archive_sha256']=='9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','source authority')
 require(type(report['containers']) is list and len(report['containers'])==2,'owned repeats')
 require(all(type(name) is str and re.fullmatch('rubix-preflight-capture-[0-9a-f]{32}-[01]',name) for name in report['containers']) and report['containers'][0].removesuffix('-0')==report['containers'][1].removesuffix('-1'),'owned names')
 require(type(report['image_id']) is str and re.fullmatch('sha256:[0-9a-f]{64}',report['image_id']),'image identity')
 for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:require(type(report[key]) is list and not report[key],'unsuccessful cleanup: '+key)
 require(report['identical_records'] is True,'repeat completion')
 require(type(report['source_sha256']) is dict and set(report['source_sha256'])==HARNESS,'harness inventory')
 for name,value in report['source_sha256'].items():require(digest(here/name)==value,'current harness: '+name)
 require(report['helper_sha256']==digest(ROOT/'tools/defaults/capture.py'),'lifecycle helper')
 require(type(report['outputs']) is dict and set(report['outputs'])=={'run0.log','run1.log','source.sha256'},'output inventory')
 for name,value in report['outputs'].items():require(digest(here/'evidence'/name)==value,'output digest')
 expected=read(here/'expected.tsv').decode().splitlines()
 require(len(expected)==44 and len(set(line.split('\t')[0] for line in expected))==44 and all(re.fullmatch('[a-z0-9_]+\t(true|false)',line) for line in expected),'expected cases')
 for name in ['run0.log','run1.log']:
  raw=read(here/'evidence'/name);require(records(raw)==[expected],'independent reference observations')
  require(len(re.findall(rb'^--- PASS: TestRubixCapture \([0-9.]+s\)$',raw,re.M))==1,'Go completion')
 source=read(here/'evidence/source.sha256').decode().splitlines()
 require(len(source)==2 and source[0]==SOURCE and re.fullmatch('[0-9a-f]{64}  /preflight.test',source[1]),'unchanged Go source and binary')
 return report
if __name__=='__main__':verify();print('Preflight policy Go oracle verified')
