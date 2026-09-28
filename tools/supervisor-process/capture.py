#!/usr/bin/env python3
"""Build exact working-tree input and qualify only inside an owned disposable PID namespace."""
import argparse
import hashlib
import json
from pathlib import Path
import selectors
import shutil
import subprocess
import tempfile
import time
import uuid
import verify

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LIMIT = 1024 * 1024

def run(argv, timeout, log, limit=LIMIT):
    deadline = time.monotonic() + timeout
    with log.open('wb') as output, subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT) as process:
        total = 0
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                while True:
                    if time.monotonic() >= deadline:
                        raise TimeoutError('bounded subprocess deadline')
                    if not selector.select(min(1, max(.001, deadline-time.monotonic()))):
                        continue
                    data = process.stdout.read1(65536)
                    if not data:
                        break
                    total += len(data)
                    if total > limit:
                        raise ValueError('bounded subprocess output exceeded')
                    output.write(data)
                return process.wait(timeout=max(.001, deadline-time.monotonic()))
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)

def short(argv):
    # The same incremental budget applies even to driver-generated control commands.
    with tempfile.TemporaryDirectory(prefix='rubix-process-control-') as tmp:
        log = Path(tmp)/'control.log'
        code = run(argv,30,log,65536)
        if code != 0: raise ValueError('owned Docker/source control command failed')
        return log.read_text().strip()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    tag = 'rubix-process-' + uuid.uuid4().hex
    owned = []
    receipt = {'schema': 1, 'source_revision': None,
               'uncommitted_source_snapshot': True, 'source_sha256': {}, 'driver_sha256': verify.digest(HERE/'capture.py'),
               'verifier_sha256': verify.digest(HERE/'verify.py'), 'runs': {}, 'cleanup_errors': [],
               'remaining_containers': None, 'remaining_images': None}
    try:
        receipt['source_revision'] = short(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'])
        with tempfile.TemporaryDirectory(prefix='rubix-process-source-') as tmp:
            context = Path(tmp)
            for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']:
                shutil.copyfile(ROOT/name, context/name)
            for name in ['crates', 'third_party', 'tools/upstream', 'tools/supervisor-process']:
                shutil.copytree(ROOT/name, context/name, ignore=shutil.ignore_patterns('target', '__pycache__', 'evidence', '.DS_Store'))
            for path in sorted(context.rglob('*')):
                if path.is_file():
                    receipt['source_sha256'][str(path.relative_to(context))] = hashlib.sha256(path.read_bytes()).hexdigest()
            (args.output/'source-inventory.json').write_text(json.dumps(receipt['source_sha256'],sort_keys=True,indent=2)+'\n')
            receipt['source_inventory_sha256']=verify.digest(args.output/'source-inventory.json')
            command = ['docker', 'build', '--platform=linux/arm64', '--tag', tag, '--file', str(context/'tools/supervisor-process/Capture.Dockerfile'), str(context)]
            receipt['build_command'] = command
            receipt['build_exit_code'] = run(command, 1800, args.output/'build.log', 16*LIMIT)
            if receipt['build_exit_code'] != 0: raise ValueError('build failed')
        receipt['image_id'] = short(['docker','image','inspect','--format','{{.Id}}',tag])
        for name in ['first','repeat']:
            container = tag + '-' + name
            owned.append(container)
            command = ['docker','run','--name',container,'--init','--network=none','--read-only',
                       '--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=96','--memory=512m',
                       '--cpus=2','--tmpfs','/tmp:rw,nosuid,nodev,size=64m',
                       '--env','RUBIX_PROCESS_DISPOSABLE=1',tag,
                       '/bin/sh','-c','sha256sum /process-tests; /process-tests --ignored --exact disposable_process_cases --nocapture; status=$?; /usr/local/bin/python3 /namespace_inventory.py || exit $?; exit "$status"']
            code=run(command, 100, args.output/(name+'.log'))
            receipt['runs'][name]={'exit_code':code,'raw_sha256':verify.digest(args.output/(name+'.log')),'command':command}
            if code != 0: raise ValueError('process qualification failed')
            records, binary = verify.raw_records(args.output/(name+'.log'))
            (args.output/(name+'.json')).write_text(json.dumps(records,sort_keys=True,indent=2)+'\n')
            receipt['runs'][name]['binary_sha256']=binary
        receipt['repeat_equal']=verify.normalize(verify.load(args.output/'first.json')) == verify.normalize(verify.load(args.output/'repeat.json'))
        if not receipt['repeat_equal']: raise ValueError('repeat semantic mismatch')
    except Exception as error:
        receipt['capture_error']=type(error).__name__ + ': ' + str(error)
        raise
    finally:
        for command in [['docker','rm','--force',name] for name in owned] + [['docker','image','rm','--force',tag]]:
            try: short(command)
            except Exception as error: receipt['cleanup_errors'].append(str(error))
        for key,command in [('remaining_containers',['docker','ps','-aq','--filter','name='+tag]),('remaining_images',['docker','images','-q',tag])]:
            try: receipt[key]=short(command).splitlines()
            except Exception as error: receipt['cleanup_errors'].append(str(error))
        (args.output/'receipt.json').write_text(json.dumps(receipt,sort_keys=True,indent=2)+'\n')
    verify.verify_capture(args.output)

if __name__ == '__main__': main()
