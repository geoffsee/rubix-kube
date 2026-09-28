"""Exercise explicit example exits; no claim of prepared host or cluster startup."""
import selectors
import subprocess
import time
from pathlib import Path

def run(argv):
    with subprocess.Popen(['/out/assess_host',*argv],stdout=subprocess.PIPE,stderr=subprocess.STDOUT) as child:
        output=bytearray();deadline=time.monotonic()+5
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout,selectors.EVENT_READ)
                while True:
                    if time.monotonic()>=deadline:raise TimeoutError('consumer deadline')
                    if not selector.select(.05):continue
                    block=child.stdout.read1(4096)
                    if not block:break
                    if len(output)+len(block)>65536:raise ValueError('consumer output bound')
                    output.extend(block)
            return child.wait(timeout=max(.001,deadline-time.monotonic())),output
        finally:
            if child.poll() is None:child.kill()
            child.wait(timeout=1)
marker=Path('/tmp/probe-started');marker.unlink(missing_ok=True)
for case,argv,expected in [('help',['--help'],0),('version',['--version'],0),('print',['--print-config'],0),('blocked',[],1)]:
    code,output=run(argv)
    if code!=expected or marker.exists():raise ValueError('consumer effect boundary '+case)
    if case=='blocked' and b'Blocked' not in output:raise ValueError('expected actual nonroot blocker')
    print('RUBIX_NODE_CONSUMER case='+case+' exit='+str(code)+' probe_absent=true')
