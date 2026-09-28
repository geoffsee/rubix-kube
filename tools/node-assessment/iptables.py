#!/usr/local/bin/python3
"""Controlled fixed-path probe double, never installed on the host."""
import os
import signal
import sys
import time
from pathlib import Path
mode=Path('/tmp/probe-mode').read_text()
Path('/tmp/probe-started').write_text('started')
if sys.argv[1:] != ['--version'] or os.getcwd() != '/' or os.environ.get('PATH') != '/usr/sbin:/usr/bin:/sbin:/bin': raise SystemExit(91)
if 'PRIVATE_SENTINEL' in os.environ: raise SystemExit(92)
if mode=='nft': os.write(2,b'iptables v1.8 (nf_tables)\n')
elif mode=='legacy': os.write(1,b'iptables v1.8 (legacy)\n')
elif mode=='invalid-utf8': os.write(1,b'\xff(nf_tables)')
elif mode=='nonzero': os.write(1,b'(nf_tables)');raise SystemExit(17)
elif mode=='overflow': os.write(1,b'x'*4097)
elif mode=='deadline':
 signal.signal(signal.SIGTERM,signal.SIG_IGN)
 time.sleep(60)
elif mode=='cancel': time.sleep(60)
elif mode=='held':
 child=os.fork()
 if child==0:
  os.setsid();Path('/tmp/held-ready').write_text('ready');time.sleep(2);os._exit(0)
 while not Path('/tmp/held-ready').exists():time.sleep(.001)
 os.write(1,b'(nf_tables)')
elif mode!='empty': raise SystemExit(93)
