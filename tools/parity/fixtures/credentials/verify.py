#!/usr/bin/env python3
"""Source-reviewed complete credential expectations; no capture-derived oracle."""
import argparse,base64,json,pathlib
HERE=pathlib.Path(__file__).resolve().parent
def load(path):
 def pairs(items):
  out={}
  for key,value in items:
   if key in out:raise ValueError('duplicate JSON key')
   out[key]=value
  return out
 def constant(value):raise ValueError('nonfinite JSON')
 return json.loads(path.read_text(),object_pairs_hook=pairs,parse_constant=constant)
def equal(actual,expected,path=''):
 if type(actual) is not type(expected):raise ValueError('type mismatch '+path)
 if isinstance(expected,dict):
  if actual.keys()!=expected.keys():raise ValueError('key inventory '+path)
  for key in expected:equal(actual[key],expected[key],path+'/'+key)
 elif isinstance(expected,list):
  if len(actual)!=len(expected):raise ValueError('length '+path)
  for i,(a,e) in enumerate(zip(actual,expected)):equal(a,e,path+'/'+str(i))
 elif actual!=expected:raise ValueError('value '+path)
def b64(text):return base64.b64encode(text.encode()).decode()
def expected(component):
 if component=='apiserver':
  config={'clusters':{'kubesolo':{'server':'https://192.0.2.8:6443','certificate-authority-data':b64('SYNTHETIC-CA')}},'users':{'kubernetes-admin':{'client-certificate-data':b64('SYNTHETIC-CERT'),'client-key-data':b64('SYNTHETIC-KEY')},'admin-token':{}},'contexts':{'kubernetes-admin@kubesolo':{'cluster':'kubesolo','user':'kubernetes-admin'},'admin-token@kubesolo':{'cluster':'kubesolo','user':'admin-token'}},'current-context':'kubernetes-admin@kubesolo'}
  checks={name:True for name in ('key_valid','restart_preserves_key','existing_corrupt_key_accepted','existing_corrupt_key_preserved','missing_parent_fails','missing_certificate_fails','kubeconfig_repeat_equal','kubeconfig_refreshes_certificate','output_directory_fails','unreadable_key_fails','baseline_static_token_matches')}
  checks.update(key_type='RSA PRIVATE KEY',key_bits=2048,key_mode='0600',kubeconfig_mode='0600')
  return {'component':component,'synthetic_kubeconfig':config,'checks':checks}
 if component=='kubelet':
  config={'clusters':{'kubernetes':{'server':'https://192.0.2.8:6443','certificate-authority':'/fixture/ca.crt'}},'users':{'system:node:fixture-node':{'client-certificate':'/fixture/node.crt','client-key':'/fixture/node.key'}},'contexts':{'system:node:fixture-node@kubernetes':{'cluster':'kubernetes','user':'system:node:fixture-node'}},'current-context':'system:node:fixture-node@kubernetes'}
  checks={name:True for name in ('repeat_equal','missing_referenced_credentials_accepted','refreshes_identity','output_directory_fails','missing_parent_fails')};checks['kubeconfig_mode']='0600'
  return {'component':component,'path_kubeconfig':config,'checks':checks}
 raise ValueError('unknown component')
def verify(value):equal(value,expected(value['component']))
def main():
 parser=argparse.ArgumentParser();parser.add_argument('directory',type=pathlib.Path);args=parser.parse_args()
 for component in ('apiserver','kubelet'):equal(load(args.directory/(component+'.json')),expected(component))
 print('credential fixtures match independent expectations')
if __name__=='__main__':main()
