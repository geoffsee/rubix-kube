#!/usr/bin/env python3
"""Report only init, this helper and its driver-shell parent after the test exits."""
import json
import os

record = {'init': 1, 'shell': os.getppid(), 'helper': os.getpid(),
          'processes': sorted(int(name) for name in os.listdir('/proc') if name.isdigit())}
print('RUBIX_NAMESPACE ' + json.dumps(record, sort_keys=True))
if set(record['processes']) != {record['init'], record['shell'], record['helper']}:
    raise SystemExit('unexpected processes remain in disposable PID namespace')
