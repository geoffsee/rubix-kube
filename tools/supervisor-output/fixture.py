"""Finite adversarial children; executed only inside the owned PID namespace."""
import os
import signal
import sys
import threading
import time
from pathlib import Path
mode, root = sys.argv[1], Path(sys.argv[2])
if mode == 'merged':
    os.write(1, b'out\xff'); os.write(2, b'err\x00')
elif mode in ('exact', 'overflow'):
    os.write(1, b'x' * (64 if mode == 'exact' else 65))
elif mode == 'simultaneous':
    worker = threading.Thread(target=lambda: os.write(2, b'b' * 1000))
    worker.start(); os.write(1, b'a' * 1000); worker.join()
elif mode == 'flood':
    while True: os.write(1, b'x' * 4096)
elif mode in ('stop', 'timeout', 'abort', 'probe-failure'):
    def stop(*_):
        os.write(2, b'term-handler-output')
        (root / 'term').write_text('handled')
        raise SystemExit(0)
    signal.signal(signal.SIGTERM, stop)
    os.write(1, b'prefix')
    (root / 'ready').write_text('ready')
    while True: time.sleep(.01)
elif mode in ('descendant', 'escaped'):
    child = os.fork()
    if child == 0:
        if mode == 'escaped': os.setsid()
        (root / 'descendant').write_text(str(os.getpid()))
        time.sleep(2)
        os._exit(0)
    while not (root / 'descendant').exists(): time.sleep(.001)
    os.write(1, b'leader')
elif mode != 'empty':
    raise SystemExit('unknown fixture mode')
