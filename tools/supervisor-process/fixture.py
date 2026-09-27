#!/usr/bin/env python3
"""Finite, synthetic process fixture. Only run inside the owned disposable container."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

mode, directory = sys.argv[1:]
root = Path(directory)
root.mkdir(parents=True, exist_ok=True)

def record(name, value):
    temporary = root / (name + '.new')
    temporary.write_text(str(value))
    temporary.replace(root / name)

if mode == 'early':
    sys.exit(17)
if mode == 'oneshot':
    sys.exit(0)

stopped = False

def term(_number, _frame):
    global stopped
    record('term', 'received')
    if mode != 'ignore':
        stopped = True

signal.signal(signal.SIGTERM, term)
child = None
if mode in ('family', 'leader-exits-first'):
    child = subprocess.Popen([sys.executable, __file__, 'ignore' if mode == 'leader-exits-first' else 'descendant', str(root / 'child')])
    deadline = time.monotonic() + 3
    while not (root / 'child/ready').exists():
        if time.monotonic() >= deadline:
            raise RuntimeError('descendant readiness timeout')
        time.sleep(.01)
if mode == 'leader-exits-first':
    sys.exit(17)
if mode == 'delayed':
    time.sleep(.2)
record('ready', os.getpid())
count = 0
while not stopped:
    count += 1
    record('heartbeat', count)
    time.sleep(.02)
if child is not None:
    # Group TERM reaches the cooperating child too; record that its parent actually reaped it.
    code = child.wait(timeout=3)
    record('descendant_reaped', code)
record('stopped', 'graceful')

if mode == 'term-error':
    sys.exit(17)
