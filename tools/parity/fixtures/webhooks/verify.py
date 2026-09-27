#!/usr/bin/env python3
"""Source-reviewed complete webhook expectations; no capture-derived oracle."""
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
def patch(path,value):return [{'op':'add','path':path,'value':value}]
def expected():
 name='webhook.kubesolo.io';configs={'missing_certificate_fails':True}
 for enabled in (False,True):
  rules=[{'operations':['CREATE'],'apiGroups':['','apps','batch'],'apiVersions':['v1'],'resources':['pods','persistentvolumeclaims','jobs']}]
  if enabled:rules.append({'operations':['CREATE','UPDATE'],'apiGroups':[''],'apiVersions':['v1'],'resources':['services']})
  configs[str(enabled).lower()]={'metadata':{'name':name},'webhooks':[{'name':name,'clientConfig':{'url':'https://127.0.0.1:10443/mutate','caBundle':base64.b64encode(b'SYNTHETIC-WEBHOOK-CERT').decode()},'rules':rules,'failurePolicy':'Ignore','sideEffects':'NoneOnDryRun','timeoutSeconds':30,'admissionReviewVersions':['v1'],'reinvocationPolicy':'IfNeeded'}]}
 nodepatch=patch('/spec/nodeName','fixture-node');pvcpatch=patch('/metadata/annotations',{'volume.kubernetes.io/selected-node':'fixture-node'});jobpatch=patch('/spec/template/spec/nodeSelector',{'kubernetes.io/hostname':'fixture-node'})
 inputs={
 'pod_unassigned':('Pod',{'metadata':{'name':'pod'},'spec':{}},nodepatch),
 'pod_assigned':('Pod',{'spec':{'nodeName':'other-node'}},None),
 'pod_update_direct':('Pod',{'spec':{}},nodepatch),
 'pvc_empty':('PersistentVolumeClaim',{'metadata':{}},pvcpatch),
 'pvc_existing_annotation':('PersistentVolumeClaim',{'metadata':{'annotations':{'keep':'value'}}},pvcpatch),
 'pvc_assigned':('PersistentVolumeClaim',{'metadata':{'annotations':{'volume.kubernetes.io/selected-node':'other-node'}}},None),
 'job_empty':('Job',{'spec':{'template':{'spec':{}}}},jobpatch),
 'job_existing_selector':('Job',{'spec':{'template':{'spec':{'nodeSelector':{'keep':'value'}}}}},jobpatch),
 'unknown_kind':('Unknown',{},None),
 'malformed_typed_object':('Pod',{'spec':'invalid'},None),
 'service_dry_run':('Service',{'metadata':{'name':'svc','namespace':'default'},'spec':{'type':'LoadBalancer'}},None),
 'service_disabled':('Service',{'metadata':{'name':'svc','namespace':'default'},'spec':{'type':'LoadBalancer'}},None),
 'service_no_address':('Service',{'metadata':{'name':'svc','namespace':'default'},'spec':{'type':'LoadBalancer'}},None),
 'service_clusterip':('Service',{'metadata':{'name':'svc','namespace':'default'},'spec':{'type':'ClusterIP'}},None),
 'wrong_content_type':('Pod',{'spec':{}},nodepatch)}
 requests={}
 for case,(kind,object_value,patches) in inputs.items():
  request={'uid':'fixture-uid','kind':{'group':'','version':'v1','kind':kind},'resource':{'group':'','version':'v1','resource':'fixture'},'operation':'UPDATE' if case=='pod_update_direct' else 'CREATE','dryRun':case=='service_dry_run','object':object_value,'oldObject':None,'options':None,'userInfo':{}}
  response={'uid':'fixture-uid','allowed':True}
  if patches:response.update(patch=base64.b64encode(json.dumps(patches,sort_keys=True,separators=(',',':')).encode()).decode(),patchType='JSONPatch')
  requests[case]={'status':200,'content_type':'application/json','scheduled_status_update':False,'admission_review':{'apiVersion':'admission.k8s.io/v1','kind':'AdmissionReview','request':request,'response':response}}
 for case,status,error in [('wrong_method',405,'method not allowed'),('malformed_body',400,"error decoding admission review: couldn't get version/kind; json parse error: unexpected end of JSON input"),('missing_request',400,'admission review with no request'),('read_failure',400,'error reading request body: synthetic read failure')]:
  requests[case]={'status':status,'content_type':'text/plain; charset=utf-8','error':error+'\n','scheduled_status_update':False}
 get={'verb':'get','resource':'services','namespace':'default','name':'svc','subresource':''}
 change={'verb':'patch','resource':'services','namespace':'default','name':'svc','subresource':'status','patch_type':'application/merge-patch+json','patch':{'status':{'loadBalancer':{'ingress':[{'ip':'192.0.2.9'}]}}}}
 statuses={}
 for case,actions,error in [('assign',[get,change],''),('already_correct',[get],''),('stale_type',[get,get,change],''),('get_failure',[get,get,change],''),('patch_failure',[get,change,get,change],''),('exhausted',[get]*5,'timed out waiting for the condition')]:
  statuses[case]={'actions':actions,'error':error,'get_count':sum(a['verb']=='get' for a in actions),'patch_count':sum(a['verb']=='patch' for a in actions)}
 return {'component':'webhook','configurations':configs,'handler_requests':requests,'fake_client_status':statuses}
def verify(value):equal(value,expected())
def main():
 parser=argparse.ArgumentParser();parser.add_argument('directory',type=pathlib.Path);args=parser.parse_args();verify(load(args.directory/'webhook.json'));print('webhook fixtures match independent expectations')
if __name__=='__main__':main()
