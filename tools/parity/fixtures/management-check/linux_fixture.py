"""Owned chroot fixture only. Never bind/mutate a host namespace or mount host paths."""
import json,os,pathlib,selectors,shutil,signal,socket,subprocess,tempfile,time

def require(ok,message):
 if not ok:raise RuntimeError(message)
def invoke(root,args,uid=0):
 def enter():
  os.chroot(root);os.chdir('/')
  if uid:os.setuid(uid)
 child=subprocess.Popen(['/rubixctl']+args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={},preexec_fn=enter,start_new_session=True)
 buffers={'stdout':bytearray(),'stderr':bytearray()};selector=selectors.DefaultSelector();deadline=time.monotonic()+5
 for key,stream in [('stdout',child.stdout),('stderr',child.stderr)]:selector.register(stream,selectors.EVENT_READ,key)
 try:
  while selector.get_map():
   require(time.monotonic()<deadline,'child deadline')
   for event,_ in selector.select(.05):
    data=os.read(event.fd,4096)
    if not data:selector.unregister(event.fileobj);continue
    require(sum(map(len,buffers.values()))+len(data)<=65536,'child output budget')
    buffers[event.data].extend(data)
  code=child.wait(timeout=max(.01,deadline-time.monotonic()))
 finally:
  if child.poll() is None:
   try:os.killpg(child.pid,signal.SIGKILL)
   except ProcessLookupError:pass
  child.wait(timeout=2);selector.close();child.stdout.close();child.stderr.close()
 return {'exit':code,**{k:bytes(v).decode() for k,v in buffers.items()}}
def free_ports():
 for port in [2379,6443,10443,6060]:
  with socket.socket(socket.AF_INET,socket.SOCK_STREAM) as s:s.bind(('0.0.0.0',port))
def main():
 rows=[]
 with tempfile.TemporaryDirectory(prefix='rubix-check-') as temporary:
  root=pathlib.Path(temporary);root.chmod(0o755);shutil.copyfile('/rubixctl',root/'rubixctl');(root/'rubixctl').chmod(0o755)
  for path,value in {'proc/version':'Linux version fixture compiler\n','proc/modules':'xt_comment 1 0 - Live 0x0\n','proc/net/ip_tables_matches':'comment\n','sys/fs/cgroup/cgroup.controllers':'cpuset cpu io memory pids\n','lib/modules/fixture/kernel/net/netfilter/xt_comment.ko':''}.items():
   target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(value)
  for name,args,uid in [('help',['check','--help'],65534),('version',['version'],65534),('root_pass',['check'],0),('nonroot',['check'],65534),('preparation',['check','--install-prereqs'],0)]:
   row=invoke(root,args,uid);row['name']=name;rows.append(row)
  with socket.socket(socket.AF_INET,socket.SOCK_STREAM) as listener:
   listener.bind(('0.0.0.0',6060));listener.listen(1)
   for name,args in [('pprof_off',['check']),('pprof_conflict',['check','--pprof-server'])]:
    row=invoke(root,args);row['name']=name;rows.append(row)
   require(listener.getsockopt(socket.SOL_SOCKET,socket.SO_ACCEPTCONN)==1,'listener preserved')
  free_ports()
  for index in range(2):
   row=invoke(root,['check','--pprof-server']);row['name']='repeat_'+str(index);rows.append(row);free_ports()
  expected={'help':0,'version':0,'root_pass':0,'nonroot':1,'preparation':1,'pprof_off':0,'pprof_conflict':1,'repeat_0':0,'repeat_1':0}
  require({r['name']:r['exit'] for r in rows}==expected,'expected exits')
  require('RootRequired' in rows[3]['stderr'],'root blocker')
  require('Port6060' in rows[6]['stderr'],'optional port conflict')
  require(all((root/path).read_text()==value for path,value in {'proc/version':'Linux version fixture compiler\n','proc/modules':'xt_comment 1 0 - Live 0x0\n','sys/fs/cgroup/cgroup.controllers':'cpuset cpu io memory pids\n'}.items()),'fixture files unchanged')
 print('RUBIX_CHECK '+json.dumps({'cases':rows,'listener_survived':True,'ports_released':True,'files_unchanged':True},sort_keys=True))
if __name__=='__main__':main()
