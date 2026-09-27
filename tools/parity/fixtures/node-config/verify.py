#!/usr/bin/env python3
"""Source-reviewed complete node configuration expectations; no capture-derived oracle."""
import argparse,hashlib,json,pathlib
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
def kubelet_config(name):
 config={'kind':'KubeletConfiguration','apiVersion':'kubelet.config.k8s.io/v1beta1','containerRuntimeEndpoint':'unix:///fixture/runtime.sock','authentication':{'anonymous':{'enabled':False},'webhook':{'enabled':True,'cacheTTL':'5m0s'},'x509':{'clientCAFile':'/fixture/ca.crt'}},'authorization':{'mode':'Webhook','webhook':{'cacheAuthorizedTTL':'10m0s','cacheUnauthorizedTTL':'1m0s'}},'clusterDomain':'cluster.local','clusterDNS':['10.43.0.10'],'resolvConf':'/etc/resolv.conf' if name=='host_default' else '/dev/null','tlsCertFile':'/fixture/node.crt','tlsPrivateKeyFile':'/fixture/node.key','cgroupDriver':'systemd' if name=='reported_systemd' else 'cgroupfs','readOnlyPort':0,'rotateCertificates':True,'failSwapOn':False}
 if name!='host_default':config.update(cgroupsPerQOS=False,enforceNodeAllocatable=[],imageGCHighThresholdPercent=100,evictionHard={'memory.available':'50Mi','nodefs.available':'0%','nodefs.inodesFree':'0%','imagefs.available':'0%'},systemReserved={},kubeReserved={})
 if name=='container_static':config.update(cpuManagerPolicy='static',reservedSystemCPUs='0-1',cpuManagerPolicyOptions={'full-pcpus-only':'true'},systemReserved={'cpu':'200m','memory':'128Mi'})
 return config
def containerd_config():
 return {'version':3,'root':'/tmp/rubix-runtime/root','state':'/fixture/state','imports':['/etc/containerd/config.d/*.toml'],'grpc':{'address':'/fixture/containerd.sock'},'plugins':{'io.containerd.cri.v1.images':{'image_pull_progress_timeout':'2m0s','pinned_images':{'sandbox':'docker.io/portainer/pause:latest'},'registry':{'config_path':'/fixture/registry'}},'io.containerd.cri.v1.runtime':{'containerd':{'default_runtime_name':'crun','runtimes':{'crun':{'runtime_type':'io.containerd.runc.v2','snapshotter':'overlayfs','options':{'BinaryName':'/fixture/crun','SystemdCgroup':False}}}},'cni':{'bin_dirs':['/fixture/cni/bin'],'conf_dir':'/etc/cni/net.d'}},'io.containerd.runtime.v2.task':{'platforms':['linux/amd64','linux/arm64','linux/arm']}}}
def verify(value,component):
 equal(value['component'],component)
 if component=='kubelet':
  equal(set(value),{'component','synthetic_resolver','variants','checkpoint_states','checkpoint_negative_control','args','failures'})
  equal(value['synthetic_resolver'],['192.0.2.53'])
  equal(set(value['variants']),{'host_default','container_default','container_static','reported_systemd','reported_cgroupfs'})
  for name,variant in value['variants'].items():
   equal(set(variant),{'config','rendered_config','yaml','repeat_equal'});equal(variant['config'],kubelet_config(name));equal(variant['rendered_config'],kubelet_config(name));equal(variant['repeat_equal'],True)
   if type(variant['yaml']) is not str or not variant['yaml']:raise ValueError('missing rendered YAML')
  equal(value['checkpoint_states'],{'same':'unchanged','policy':'absent','options':'absent','reserved':'absent','unrelated':'unchanged','malformed_previous':'absent'});equal(value['checkpoint_negative_control'],'changed')
  args={}
  for ip in ('192.0.2.8','2001:db8::8','127.0.0.1','bad'):
   args[ip]=['--config','/tmp/rubix-node/kubelet.yaml','--hostname-override','fixture-node','--root-dir','/tmp/rubix-node','--kubeconfig','/fixture/node.kubeconfig']+(['--node-ip',ip] if ip in ('192.0.2.8','2001:db8::8') else [])
  equal(value['args'],args);equal(value['failures'],{'output_directory':True,'parent_file':True})
 elif component=='containerd':
  equal(set(value),{'component','config','rendered_config','toml','checks'});equal(value['config'],containerd_config());equal(value['rendered_config'],containerd_config())
  equal(value['checks'],{'repeat_equal':True,'tmpfs_snapshotter':'overlayfs','missing_parent_snapshotter':'overlayfs','systemd_cgroup':False,'output_directory_fails':True,'missing_parent_fails':True})
  if type(value['toml']) is not str or not value['toml']:raise ValueError('missing rendered TOML')
 else:raise ValueError('unknown component')
def main():
 parser=argparse.ArgumentParser();parser.add_argument('directory',type=pathlib.Path);args=parser.parse_args();provenance=load(HERE/'provenance.json')
 for component in ('kubelet','containerd'):
  actual=load(args.directory/(component+'.json'));verify(actual,component)
  path=HERE/'expected'/ (component+'.json')
  if hashlib.sha256(path.read_bytes()).hexdigest()!=provenance['expected_sha256'][component]:raise ValueError('frozen expected identity mismatch')
  frozen=load(path);verify(frozen,component);equal(actual,frozen)
 print('node configuration matches independent semantics and hash-bound rendered fixture')
if __name__=='__main__':main()
