import json,math,hashlib,pathlib,re
HERE=pathlib.Path(__file__).resolve().parent;ROOT=HERE.parents[3]
INPUTS=['Capture.Dockerfile','extract.go','go.mod','capture_test.go','cases.json','capture.py','verify.py','expected.json','source-pins.json']
def require(ok,message):
 if not ok:raise ValueError(message)
def strict(raw):
 def pairs(items):
  result={}
  for k,v in items:require(k not in result,'duplicate JSON key');result[k]=v
  return result
 def number(s):
  v=float(s);require(math.isfinite(v),'nonfinite JSON');return v
 def constant(_):raise ValueError('nonfinite JSON')
 return json.loads(raw,object_pairs_hook=pairs,parse_float=number,parse_constant=constant)
def read(path):
 with pathlib.Path(path).open('rb') as f:raw=f.read(1048577)
 require(len(raw)<=1048576,'byte budget');return raw
def load(path):return strict(read(path))
def digest(path):return hashlib.sha256(read(path)).hexdigest()
def equal(a,b,message):require(json.dumps(a,sort_keys=True,allow_nan=False)==json.dumps(b,sort_keys=True,allow_nan=False),message)
def records(path):
 rows=[strict(l[14:]) for l in read(path).splitlines() if l.startswith(b'RUBIX_CAPTURE ')]
 require(len(rows)==1,'exact record count');return rows[0]
def classify(row):
 require(type(row) is dict and set(row)=={'name','stdout','stderr','exit'},'record schema')
 require(all(type(row[k]) is str for k in ['name','stdout','stderr']) and type(row['exit']) is int,'record types')
 result={'name':row['name']};out,err,code=row['stdout'],row['stderr'],row['exit']
 if code==1:
  require(out=='' and err.startswith('error: '),'error channel')
  if 'unknown flag:' in err or 'unknown shorthand flag:' in err:result['outcome']='unknown_flag'
  elif 'strconv.ParseBool:' in err:result['outcome']='invalid_boolean'
  elif 'unknown command ' in err:result['outcome']='unknown_command'
  else:raise ValueError('unrecognized parser error')
 elif code==0:
  if err:
   require(out=='' and err.startswith('Unknown help topic '),'unknown help channel');result['outcome']='unknown_help'
  elif out.startswith('CHECK '):
   match=re.fullmatch(r'CHECK (true|false) (true|false)\n',out);require(match is not None,'effect marker')
   result.update(outcome='check',install=match[1]=='true',pprof=match[2]=='true')
  elif out=='kubesoloctl dev (commit unknown, built unknown)\n':result['outcome']='version'
  elif out.startswith('Validates that this host meets all requirements for KubeSolo.'):
   require('  kubesoloctl check [flags]' in out and '--install-prereqs' in out and '--pprof-server' in out,'check help');result['outcome']='help_check'
  elif out.startswith('Print kubesoloctl version information'):result['outcome']='help_version'
  elif out.startswith('kubesoloctl manages the lifecycle'):result['outcome']='help_root'
  else:raise ValueError('unrecognized output')
 else:raise ValueError('exit status')
 return result
def verify(directory):
 directory=pathlib.Path(directory);receipt=load(directory/'receipt.json')
 require(type(receipt) is dict and set(receipt)=={'revision','archive_sha256','source_sha256','helper_sha256','containers','errors','cleanup_errors','outputs','image_id','identical_records','remaining_containers','remaining_images'},'receipt inventory')
 require(type(receipt['image_id']) is str and re.fullmatch('sha256:[a-f0-9]{64}',receipt['image_id']) is not None,'image identity')
 require(type(receipt['containers']) is list and len(receipt['containers'])==2 and all(type(n) is str and re.fullmatch('rubix-management-[a-f0-9]{32}-[01]',n) for n in receipt['containers']) and receipt['containers'][1]==receipt['containers'][0][:-1]+'1','owned container identity')
 equal(receipt['revision'],'2ef1c4787989f11f868f81bb84ae2afd4a49a81d','revision')
 equal(receipt['archive_sha256'],'9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','archive')
 equal(receipt['source_sha256'],{n:digest(HERE/n) for n in INPUTS},'source inventory')
 equal(receipt['helper_sha256'],digest(ROOT/'tools/defaults/capture.py'),'helper')
 for key in ['errors','cleanup_errors','remaining_images','remaining_containers']:equal(receipt[key],[],key)
 require(receipt['identical_records'] is True,'repeated capture')
 equal(set_to_list(receipt['outputs']),['run0.log','run1.log','source.sha256'],'output inventory')
 for name,value in receipt['outputs'].items():equal(digest(directory/name),value,'output digest')
 a,b=records(directory/'run0.log'),records(directory/'run1.log');equal(a,b,'exact raw repeat')
 expected=load(HERE/'expected.json');equal([classify(r) for r in a],expected,'independent parser expectations')
 equal([c['name'] for c in load(HERE/'cases.json')],[r['name'] for r in expected],'case identity/order')
 pins=load(HERE/'source-pins.json');hashes={}
 for line in read(directory/'source.sha256').decode().splitlines():
  match=re.fullmatch(r'([a-f0-9]{64})  (.+)',line);require(match is not None,'hash row');require(match[2] not in hashes,'duplicate hash');hashes[match[2]]=match[1]
 equal(sorted(hashes),sorted(list(pins)+['/capture.test']),'source/binary inventory');equal({k:hashes[k] for k in pins},pins,'baseline source pins')
 return receipt
def set_to_list(value):require(type(value) is dict,'object required');return sorted(value)
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('directory',type=pathlib.Path);a=p.parse_args();verify(a.directory);print('PASS: actual Cobra observations, exact inputs and cleanup')
