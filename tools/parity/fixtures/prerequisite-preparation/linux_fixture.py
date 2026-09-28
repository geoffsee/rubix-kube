"""Synthetic command effects in an owned chroot, never the host or real package database."""
import json,os,pathlib,selectors,shutil,signal,subprocess,tempfile,time

def require(ok,message):
 if not ok:raise RuntimeError(message)
def setup(root,mode=''):
 root.chmod(0o755)
 for path in ['sbin','dev','proc/net','sys/fs/cgroup','etc']: (root/path).mkdir(parents=True,exist_ok=True)
 shutil.copyfile('/rubixctl',root/'rubixctl');(root/'rubixctl').chmod(0o755)
 for name in ['apk','rc-update','rc-service']:
  shutil.copyfile('/fixture-command',root/'sbin'/name);(root/'sbin'/name).chmod(0o755)
 for path,text in {'dev/null':'','etc/alpine-release':'3.24.2\n','proc/version':'Linux fixture\n','proc/modules':'xt_comment 1 0 - Live 0x0\n','proc/net/ip_tables_matches':'comment\n','mode':mode}.items():(root/path).write_text(text)
def invoke(root,args,send=None,uid=0):
 def enter():
  os.chroot(root);os.chdir('/')
  if uid:os.setuid(uid)
 child=subprocess.Popen(['/rubixctl']+args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={},preexec_fn=enter,start_new_session=True)
 streams={'stdout':bytearray(),'stderr':bytearray()};selector=selectors.DefaultSelector();deadline=time.monotonic()+10;sent=False;owned=None
 for key,stream in [('stdout',child.stdout),('stderr',child.stderr)]:selector.register(stream,selectors.EVENT_READ,key)
 try:
  while selector.get_map():
   require(time.monotonic()<deadline,'CLI deadline')
   if send and not sent and (root/'active-pid').exists():
    owned=int((root/'active-pid').read_text());os.kill(child.pid,send);time.sleep(.01)
    if child.poll() is None:os.kill(child.pid,signal.SIGTERM if send==signal.SIGINT else signal.SIGINT)
    sent=True
   for event,_ in selector.select(.01):
    data=os.read(event.fd,4096)
    if not data:selector.unregister(event.fileobj);continue
    require(sum(map(len,streams.values()))+len(data)<=65536,'CLI output limit');streams[event.data].extend(data)
  code=child.wait(timeout=max(.01,deadline-time.monotonic()))
 finally:
  if child.poll() is None:
   try:os.killpg(child.pid,signal.SIGKILL)
   except ProcessLookupError:pass
  child.wait(timeout=2);selector.close();child.stdout.close();child.stderr.close()
 if send:
  require(sent and owned is not None,'active child observed before signal')
  try:os.kill(owned,0)
  except ProcessLookupError:pass
  else:raise RuntimeError('owned command survives CLI')
 actions=(root/'actions').read_text().splitlines() if (root/'actions').exists() else []
 return {'exit':code,'actions':actions,'signal_sent':int(send) if send else None,'owned_child_absent':bool(send),**{key:bytes(data).decode() for key,data in streams.items()}}
def main():
 rows=[]
 cases=[('opt_out','',False,None,0),('prepare','',True,None,0),('no_effect_success','noop',True,None,0),('register_failure','fail-update',True,None,0),('nonroot','',True,None,65534)]
 cases += [('signal_'+str(i),'hold',True,signal.SIGINT if i%2==0 else signal.SIGTERM,0) for i in range(20)]
 for name,mode,optin,send,uid in cases:
  with tempfile.TemporaryDirectory(prefix='rubix-preparation-') as temporary:
   root=pathlib.Path(temporary);setup(root,mode)
   row=invoke(root,['check']+(['--install-prereqs'] if optin else []),send,uid);row['name']=name;rows.append(row)
   if name=='prepare':
    repeat=invoke(root,['check','--install-prereqs']);repeat['name']='repeat';rows.append(repeat)
 print('RUBIX_PREPARATION '+json.dumps({'cases':rows},sort_keys=True))
if __name__=='__main__':main()
