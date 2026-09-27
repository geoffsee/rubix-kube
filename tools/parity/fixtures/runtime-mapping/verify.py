#!/usr/bin/env python3
"""Independent source-specified runtime mapping assertions; no generated-output oracle."""
import argparse
import json
import math
import pathlib
import posixpath

LIMIT=1024*1024

def pairs(items):
 out={}
 for key,value in items:
  if key in out:raise ValueError('duplicate JSON key')
  out[key]=value
 return out

def reject(value):raise ValueError('nonfinite JSON constant')
def finite_float(text):
 value=float(text)
 if not math.isfinite(value):raise ValueError("nonfinite JSON number")
 return value
def loads(text):
 if len(text.encode())>LIMIT:raise ValueError('JSON exceeds limit')
 return json.loads(text,object_pairs_hook=pairs,parse_constant=reject,parse_float=finite_float)
def load(path):
 with pathlib.Path(path).open('rb') as source:data=source.read(LIMIT+1)
 if len(data)>LIMIT:raise ValueError('JSON exceeds limit')
 return loads(data.decode())
def equal(actual,expected,path=''):
 if type(actual) is not type(expected):raise ValueError('type differs at '+path)
 if isinstance(expected,dict):
  if actual.keys()!=expected.keys():raise ValueError('keys differ at '+path)
  for key in expected:equal(actual[key],expected[key],path+'/'+key)
 elif isinstance(expected,list):
  if len(actual)!=len(expected):raise ValueError('length differs at '+path)
  for index,(a,b) in enumerate(zip(actual,expected)):equal(a,b,path+'/'+str(index))
 elif actual!=expected:raise ValueError('value differs at '+path)

def inputs():
 rows=[('default','/var/lib/kubesolo','','fixture-node','',False),('custom','/fixture/a/../state//','  Talos-CP-1  ','unused','',True),('empty-path','','','fixture-node','',False),('relative-path','relative/../state','','fixture-node','',False),('raw-hostname','/fixture',' ',' MIXED-Host ','',False),('empty-hostname','/fixture','','','',False),('external-path','/fixture','','fixture-node','  /run/crio/crio.sock  ',True),('external-url','/fixture','','fixture-node','unix:///run/a/../runtime.sock',False),('whitespace-endpoint','/fixture','','fixture-node',' \t ',False),('relative-endpoint','/fixture','','fixture-node','run/runtime.sock',False),('non-unix-endpoint','/fixture','','fixture-node','tcp://127.0.0.1:1234',False),('unix-host-endpoint','/fixture','','fixture-node','unix://localhost/run/runtime.sock',False),('root-endpoint','/fixture','','fixture-node','unix:///',False)]
 return [dict(zip(('id','path','node_name','hostname','endpoint','changed'),row)) for row in rows]

# Explicit reviewed suffix inventory from BuildEmbedded, not inferred from captured output.
PATHS={
 'AdminKubeconfigFile':'pki/admin/admin.kubeconfig',
 'PKIDir':'pki','PKICADir':'pki/ca','PKIAdminDir':'pki/admin','PKIAPIServerDir':'pki/apiserver','PKIControllerDir':'pki/controller-manager','PKIKubeletDir':'pki/kubelet','PKIWebhookDir':'pki/webhook','PKIRequestHeaderDir':'pki/request-header',
 'ContainerdDir':'containerd','ContainerdSocketFile':'containerd/containerd.sock','ContainerdBinaryFile':'containerd/containerd','ContainerdImagesDir':'containerd/images','ContainerdShimBinaryFile':'containerd/containerd-shim-runc-v2','ContainerdConfigFile':'containerd/config.toml','ContainerdRootDir':'containerd/root','ContainerdStateDir':'containerd/state','ContainerdRegistryConfigDir':'containerd/registry','ContainerdCNIDir':'containerd/cni','ContainerdCNIPluginsDir':'containerd/cni/plugins','ContainerdCNIConfigDir':'containerd/cni/conf','ContainerdCNIConfigFile':'containerd/cni/conf/10-bridge.conflist','CrunBinaryFile':'containerd/crun',
 'KubeletDir':'kubelet','KubeletConfigDir':'kubelet/config','KubeletConfigFile':'kubelet/config/config.yaml','KubeletKubeConfigFile':'pki/kubelet/kubelet.kubeconfig','KubeletPluginsDir':'kubelet/volumeplugins','APIServerDir':'apiserver','ServiceAccountKeyFile':'pki/apiserver/service-account.key','KineDir':'kine/db','KineSocketFile':'kine/db/socket','ControllerDir':'controller-manager/config','WebhookDir':'pki/webhook',
 'PortainerEdgeImageFile':'containerd/images/portainer-agent.tar.gz','CorednsImageFile':'containerd/images/coredns.tar.gz','SandboxImageFile':'containerd/images/pause.tar.gz','LocalPathProvisionerImageFile':'containerd/images/local-path-provisioner.tar.gz','D2KImageFile':'containerd/images/d2k.tar.gz','LocalPathStorageDir':'local-path-storage'}

def expected():
 out=[]
 for spec in inputs():
  endpoint=spec['endpoint'].strip();socket=endpoint.removeprefix('unix://')
  external=bool(endpoint);resolved={'URL':'','SocketPath':'','External':False}
  record={'input':spec,'resolved':resolved,'error':'','embedded':None}
  if external and not socket.startswith('/'):
   record['error']='invalid container runtime endpoint '+json.dumps(endpoint)+': expected an absolute socket path or a unix:// URL, for example unix:///run/crio/crio.sock'
   out.append(record);continue
  if external:resolved.update(URL='unix://'+socket,SocketPath=socket,External=True)
  join=lambda suffix:posixpath.normpath(posixpath.join(spec['path'],suffix))
  value={key:join(suffix) for key,suffix in PATHS.items()}
  for key,component in [('KubeletCerts','kubelet'),('APIServerCerts','apiserver'),('ControllerManagerCerts','controller-manager'),('AdminCerts','admin'),('WebhookCerts','webhook')]:
   value[key]={'CACert':join('pki/ca/ca.crt'),'Cert':join('pki/'+component+'/'+component+'.crt'),'Key':join('pki/'+component+'/'+component+'.key')}
  value['CACerts']={'Cert':join('pki/ca/ca.crt'),'Key':join('pki/ca/ca.key')}
  value['RequestHeaderCerts']={key:join('pki/request-header/'+suffix) for key,suffix in [('CACert','request-header-ca.crt'),('CAKey','request-header-ca.key'),('ClientCert','request-header-client.crt'),('ClientKey','request-header-client.key')]}
  value['D2KCerts']={key:join(suffix) for key,suffix in [('CACert','pki/ca/ca.crt'),('ServerCert','pki/d2k/server.crt'),('ServerKey','pki/d2k/server.key'),('ClientCert','pki/d2k/client.crt'),('ClientKey','pki/d2k/client.key')]}
  changed=spec['changed'];runtime_socket=socket if external else value['ContainerdSocketFile']
  value.update(NodeName=spec['node_name'].strip().lower() or spec['hostname'],NodeIP='2001:db8::10' if changed else '192.0.2.10',NodeIPSpecified=changed,MTU=1280 if changed else 1450,MTUSpecified=changed,RuntimeExternal=external,RuntimeEndpoint='unix://'+runtime_socket,RuntimeSocketPath=runtime_socket,RuntimeCgroupDriver='',APIServerExtraSANs=['fixture.example','192.0.2.55'] if changed else None,LoadBalancer=not changed,LoadBalancerIP='2001:db8::11' if changed else '192.0.2.11',LocalStorage=not changed,IsPortainerEdge=changed,PortainerEdgeImage='fixture.invalid/agent:custom' if changed else 'docker.io/portainer/agent:lts',ContainerMode=changed,DisableIPv6=changed,D2K=changed,D2KNamespace='fixture-d2k' if changed else 'd2k',Metrics={'enabled':changed,'bindAddress':'127.0.0.1:19105' if changed else '127.0.0.1:9105'},CPUManager={'policy':'static','reservedCPUs':'0-1','policyOptions':{'full-pcpus-only':'true'}} if changed else {'policy':'none','reservedCPUs':''},SystemReserved={'cpu':'200m','memory':'128Mi'} if changed else None)
  record['embedded']=value;out.append(record)
 return out

def verify(value):equal(value,expected())
def main():
 parser=argparse.ArgumentParser();parser.add_argument('capture',type=pathlib.Path);args=parser.parse_args();verify(load(args.capture))
if __name__=='__main__':main()
