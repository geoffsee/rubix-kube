#!/usr/bin/env python3
"""Owned disposable guest observer; no CRI protocol or host runtime invocation."""
import hashlib,json,os,signal,socket,stat,subprocess,sys,time
from pathlib import Path
BUNDLE=Path('/tmp/rubix-bundle')
EXTERNAL=Path('/tmp/external-runtime')
CONTROLS=[f'/proc/sys/net/ipv6/conf/{name}/disable_ipv6' for name in ('all','default','lo')]
LAUNCHER=['/usr/bin/unshare','--mount','--propagation','private']
LIMIT=1024*1024
DEADLINE=time.monotonic()+480

def read(path,limit=LIMIT):
    with open(path,'rb') as stream:raw=stream.read(limit+1)
    if len(raw)>limit:raise RuntimeError('observation budget')
    return raw

def sha(path):return hashlib.sha256(read(path,32*1024*1024)).hexdigest()
def emit(event,**fields):print(json.dumps(dict(schema=1,event=event,**fields),sort_keys=True),flush=True)
def ensure(ok,message):
    if not ok:raise RuntimeError(message)
def identity(pid):
    raw=read(f'/proc/{pid}/stat',8192).decode();tail=raw[raw.rfind(') ')+2:].split()
    ensure(len(tail)>19 and tail[0] not in ['Z','X'],'live process')
    return dict(pid=pid,starttime=int(tail[19]),exe_sha256=sha(f'/proc/{pid}/exe'),mnt=os.readlink(f'/proc/{pid}/ns/mnt'),cgroup=read(f'/proc/{pid}/cgroup',8192).decode())
def file_identity(path):
    s=os.lstat(path)
    return dict(device=s.st_dev,inode=s.st_ino,mode=s.st_mode,uid=s.st_uid,gid=s.st_gid)
def external():
    out=identity(keeper.pid);out['socket']=file_identity(EXTERNAL/'containerd.sock')
    ensure(stat.S_ISSOCK(out['socket']['mode']),'external socket remains socket')
    out['configuration']=dict(identity=file_identity(EXTERNAL/'config.toml'),sha256=sha(EXTERNAL/'config.toml'))
    return out

def sentinels():
    result={}
    for root in ['/etc/init.d','/etc/cni/net.d','/tmp/external-runtime']:
        p=Path(root)
        if not p.exists():result[root]=None;continue
        paths=sorted(p.iterdir());ensure(len(paths)<=256,'sentinel inventory bound')
        values={}
        for entry in paths:
            data=file_identity(entry)
            if entry.is_symlink():data['link']=os.readlink(entry)
            elif entry.is_file():data['sha256']=sha(entry)
            values[entry.name]=data
        result[root]=values
    return result

def scalar_values(prefix=''):
    values=[read(prefix+path,64).decode() for path in CONTROLS]
    ensure(all(v in ['0','0\n','1','1\n'] for v in values),'kernel scalar grammar')
    return [int(v) for v in values]

def observe(process,phase,case):
    value=dict(identity=identity(process.pid),external=external(),sentinels=sentinels(),
      observer_mnt=os.readlink('/proc/self/ns/mnt'),root_enabled=read('/sys/fs/cgroup/cgroup.subtree_control',4096).decode(),observer_mounts=read('/proc/self/mountinfo',256*1024).decode(),
      mounts=read(f'/proc/{process.pid}/mountinfo',256*1024).decode(),
      visible_values=scalar_values(f'/proc/{process.pid}/root'),outside_values=scalar_values())
    emit('observation',case=case,phase=phase,**value)
    return value

def events(path,complete=False):
    raw=read(path)
    if complete:ensure(raw.endswith(b'\n'),'complete consumer records')
    lines=raw.splitlines(keepends=True)
    return [json.loads(line) for line in lines if line.endswith(b'\n')]

def wait_event(process,path,event):
    end=min(DEADLINE,time.monotonic()+180)
    while time.monotonic()<end:
        if any(row.get('event')==event for row in events(path)):return
        ensure(process.poll() is None,'candidate exited before '+event)
        time.sleep(.02)
    raise RuntimeError('candidate event deadline '+event)

def wait_file(process,path):
    end=min(DEADLINE,time.monotonic()+10)
    while time.monotonic()<end:
        if path.exists():return
        ensure(process.poll() is None,'candidate exited before double')
        time.sleep(.01)
    raise RuntimeError('module double start deadline')

def settle(process):
    # Parent retains original Popen ownership. Cancellation is requested, then awaited.
    # Hard escalation is fixture failure, never converted into successful semantics.
    if process.poll() is None:
        process.send_signal(signal.SIGTERM)
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill();process.wait(timeout=5)
            raise RuntimeError('candidate did not settle cancellation')

def failure_log(path):
    try:
        with open(path,'rb') as stream:raw=stream.read(65537)
        return dict(text=raw[:65536].decode(errors='replace'),truncated=len(raw)>65536,error=None)
    except OSError as error:
        return dict(text='',truncated=False,error=type(error).__name__)

def report_failure(case,process,out,err,error):
    cleanup_error=None
    if process is not None:
        try:settle(process)
        except BaseException as cleanup:cleanup_error=type(cleanup).__name__+': '+str(cleanup)
    emit('consumer_failure',case=case,pid=None if process is None else process.pid,
         kind=type(error).__name__,message=str(error),cleanup_error=cleanup_error,
         stdout=failure_log(out),stderr=failure_log(err))

def run_case(case,value):
    for path in CONTROLS:Path(path).write_text(str(value))
    ensure(scalar_values()==[value]*3,'explicit fixture initial sysctls')
    before=dict(external=external(),sentinels=sentinels(),observer_mnt=os.readlink('/proc/self/ns/mnt'),root_enabled=read('/sys/fs/cgroup/cgroup.subtree_control',4096).decode(),
                observer_mounts=read('/proc/self/mountinfo',256*1024).decode(),outside_values=scalar_values())
    emit('before',case=case,**before)
    out=Path('/tmp/constrained-'+case+'.out');err=Path('/tmp/constrained-'+case+'.err')
    process=None
    try:
        with out.open('wb') as stdout,err.open('wb') as stderr:
            process=subprocess.Popen(LAUNCHER+['/bin/sh',str(BUNDLE/'namespace.sh'),case],stdin=subprocess.PIPE,stdout=stdout,stderr=stderr)
            wait_event(process,out,'READY');observe(process,'READY',case)
            process.stdin.write(b'G');process.stdin.flush()
            if case=='cancel':
                marker=Path('/tmp/constrained-module-double.pid');wait_file(process,marker)
                pid_text,module=read(marker,256).decode().strip().split();double_pid=int(pid_text)
                emit('double_started',case=case,module=module,identity=identity(double_pid))
                process.send_signal(signal.SIGTERM)
            elif case!='guard':
                wait_event(process,out,'FIRST');observe(process,'FIRST',case)
                process.stdin.write(b'R');process.stdin.flush()
                wait_event(process,out,'SECOND');observe(process,'SECOND',case)
                process.stdin.write(b'Q');process.stdin.flush()
            code=process.wait(timeout=30)
            process.stdin.close()
        emit('consumer',case=case,pid=process.pid,exit=code,events=events(out,complete=True),stderr=read(err).decode())
        ensure(not Path(f'/proc/{process.pid}').exists(),'candidate reaped')
        if case=='guard':emit('guard_calls',value=read('/tmp/constrained-guard-double.calls',256).decode())
        if case=='cancel':
            ensure(not Path(f'/proc/{double_pid}').exists(),'owned module double absent')
            emit('double_absent',pid=double_pid)
        after=dict(external=external(),sentinels=sentinels(),observer_mnt=os.readlink('/proc/self/ns/mnt'),root_enabled=read('/sys/fs/cgroup/cgroup.subtree_control',4096).decode(),
                   observer_mounts=read('/proc/self/mountinfo',256*1024).decode(),outside_values=scalar_values())
        ensure(before==after,'outside resources unchanged')
        emit('after',case=case,pid_absent=process.pid,**after)
    except BaseException as error:
        report_failure(case,process,out,err,error)
        raise

def run_keeper():
    # Socket is only an ownership sentinel: accepts no requests and speaks no CRI.
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as listener:
        listener.bind(str(EXTERNAL/'containerd.sock'));listener.listen(1)
        print('READY',flush=True)
        sys.stdin.buffer.read(1)
    sys.exit(0)

keeper=None
def main():
    global keeper
    try:
        EXTERNAL.mkdir();(EXTERNAL/'config.toml').write_text('owned external-runtime sentinel\n')
        keeper=subprocess.Popen([sys.executable,__file__,'--keeper'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        import select
        ensure(select.select([keeper.stdout],[],[],10)[0],'keeper startup deadline')
        ensure(keeper.stdout.readline(64)==b'READY\n','keeper readiness')
        emit('setup',launcher_argv=LAUNCHER,unshare_version=subprocess.check_output(['/usr/bin/unshare','--version'],timeout=5,text=True).strip(),
             unshare_sha256=sha('/usr/bin/unshare'),candidate_sha256=sha(BUNDLE/'prepare_node_host'),external=external())
        flags=['--no-container-mode','--disable-ipv6','--container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock','--print-config']
        with open('/tmp/constrained-cli.out','wb') as stdout,open('/tmp/constrained-cli.err','wb') as stderr:
            cli=subprocess.run([str(BUNDLE/'prepare_node_host'),*flags],stdout=stdout,stderr=stderr,timeout=10)
        emit('cli',argv=flags,exit=cli.returncode,stdout=read('/tmp/constrained-cli.out').decode(),stderr=read('/tmp/constrained-cli.err').decode())
        ensure(cli.returncode==0,'exact guest flag grammar')
        for case,value in [('correct',1),('needs_write',0),('guard',0),('cancel',0)]:run_case(case,value)
        keeper.stdin.close();ensure(keeper.wait(timeout=5)==0,'keeper orderly exit')
        ensure(not Path(f'/proc/{keeper.pid}').exists(),'keeper reaped')
        os.unlink(EXTERNAL/'containerd.sock')
        emit('complete',keeper_pid_absent=keeper.pid,socket_removed=True)
    except BaseException as error:
        emit('failure',kind=type(error).__name__,message=str(error))
        raise
    finally:
        if keeper is not None and keeper.poll() is None:
            keeper.stdin.close()
            try:keeper.wait(timeout=5)
            except subprocess.TimeoutExpired:keeper.kill();keeper.wait(timeout=5)

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='--keeper':run_keeper()
    else:main()
