#!/usr/bin/env python3
"""Independent selected completion semantics plus hash-bound complete frozen comparison."""
import argparse, hashlib, json, pathlib
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
  for i,(a,b) in enumerate(zip(actual,expected)):equal(a,b,path+'/'+str(i))
 elif actual!=expected:raise ValueError('value mismatch '+path)

def verify(value):
 equal(value['schema_version'],1)
 equal(value['runtime'],{'go':'go1.26.8','os':'linux','arch':'arm64'})
 equal(value['controls'],{'bind_address':'127.0.0.1','external_address':'127.0.0.1','cert_directory':'','operation':'ServerRunOptions.Complete','server_started':False})
 cases=value['cases']
 equal(sorted(cases),sorted(['default','dual_stack','invalid_cidr','small_cidr','invalid_watch_cache','invalid_token_expiration']))
 errors={'invalid_cidr':'service-cluster-ip-range[0] is not a valid cidr','small_cidr':'error determining service IP ranges for primary service cidr: the service cluster IP range must be at least 8 IP addresses','invalid_watch_cache':'invalid size of watch cache size: pods#invalid','invalid_token_expiration':'the service-account-max-token-expiration must be between 1 hour and 2^32 seconds'}
 for name,error in errors.items():equal(cases[name],{'error':error},name)
 for name in ['default','dual_stack']:
  case=cases[name];dual=name=='dual_stack'
  expected={'primary_service_cidr':'10.96.0.0/20' if dual else '10.0.0.0/24','secondary_service_cidr':'fd00:1234::/108' if dual else '<nil>','service_ip':'10.96.0.1' if dual else '10.0.0.1','advertise_address':'127.0.0.1','external_host':'127.0.0.1','authorization_modes':['Node','RBAC'] if dual else ['AlwaysAllow'],'anonymous_auth':dual,'watch_cache_sizes':['events#0','events.events.k8s.io#0']+(['pods#42'] if dual else []),'events_history_window':'2m15s' if dual else '1m15s','runtime_config':{'/v1':'true','apps/v1':'true'} if dual else {},'token_max_expiration':'2h0m0s' if dual else '0s','generated_serving_certificate':True,'serving_cert_file':'','serving_key_file':'','listener_created':False}
  for key,want in expected.items():equal(case[key],want,name+'/'+key)
  before=case['flags_before'];after=case['flags_after']
  equal(sorted(before),sorted(after))
  for flags in [before,after]:
   for key,item in flags.items():
    if type(key) is not str or type(item) is not str:raise ValueError('flag names/values must be strings')
  for key,want in {'advertise-address':'<nil>','external-hostname':'','authorization-mode':'[Node,RBAC]' if dual else '[]','anonymous-auth':'true'}.items():equal(before[key],want,name+'/before/'+key)
  for key,want in {'advertise-address':'127.0.0.1','external-hostname':'127.0.0.1','authorization-mode':'[Node,RBAC]' if dual else '[AlwaysAllow]','anonymous-auth':'true' if dual else 'false'}.items():equal(after[key],want,name+'/after/'+key)

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('capture',type=pathlib.Path);args=parser.parse_args()
 value=load(args.capture);verify(value)
 frozen=HERE/'expected.json';provenance=load(HERE/'provenance.json')
 if hashlib.sha256(frozen.read_bytes()).hexdigest()!=provenance['expected_sha256']:raise ValueError('frozen fixture identity mismatch')
 expected=load(frozen);verify(expected);equal(value,expected)
 print('resolved options match source expectations and complete reviewed fixture')
if __name__=='__main__':main()
