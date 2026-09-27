#!/usr/bin/env python3
"""Compile additive real-baseline harnesses; run without host mounts or privileges."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tempfile
import uuid
import verify
import selectors
import time

HERE = pathlib.Path(__file__).resolve().parent

def run(argv, timeout, **kwargs):
    destination = kwargs.get('stdout')
    if not hasattr(destination, 'write'):
        return subprocess.run(argv, timeout=timeout, check=True, **kwargs)
    # Drain incrementally so even diagnostic output has a finite host budget.
    limit = 16 * 1024 * 1024 if 'build' in argv else 1024 * 1024
    kwargs['stdout'] = subprocess.PIPE
    deadline = time.monotonic() + timeout
    with subprocess.Popen(argv, **kwargs) as process:
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                size = 0
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise subprocess.TimeoutExpired(argv, timeout)
                    if not selector.select(min(remaining, 1)):
                        continue
                    chunk = process.stdout.read1(65536)
                    if not chunk:
                        break
                    size += len(chunk)
                    if size > limit:
                        raise ValueError('subprocess output exceeds bounded log budget')
                    destination.write(chunk)
                code = process.wait(timeout=max(0.001, deadline - time.monotonic()))
                if code:
                    raise subprocess.CalledProcessError(code, argv)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)


def cleanup_and_receipt(report, tag, containers, output):
    for label, command in [(name, ['docker', 'rm', '--force', name]) for name in containers] + [('image', ['docker', 'image', 'rm', '--force', tag])]:
        try:
            run(command, 30, stdout=subprocess.DEVNULL)
        except Exception as error:
            report['cleanup_errors'].append(label + ': ' + str(error))
    for key, command in [('remaining_containers', ['docker', 'ps', '-aq', '--filter', 'name=' + tag]), ('remaining_images', ['docker', 'images', '-q', tag])]:
        report[key] = None  # Unknown must not be mistaken for a verified empty inventory.
        try:
            report[key] = subprocess.check_output(command, timeout=30).decode().splitlines()
        except Exception as error:
            report['cleanup_errors'].append(key + ': ' + str(error))
    # Inventory/cleanup failures cannot bypass publication of their evidence.
    (output / 'receipt.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    tag = 'rubix-oracles-' + uuid.uuid4().hex
    containers = []
    report = {'schema': 1, 'reference_revision': '2ef1c4787989f11f868f81bb84ae2afd4a49a81d', 'source_archive_sha256': '9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec', 'builder': 'golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd', 'capture_driver_sha256': hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(), 'verifier_sha256': hashlib.sha256((HERE / 'verify.py').read_bytes()).hexdigest(), 'replacement_sha256': hashlib.sha256((HERE / 'replacement.yaml').read_bytes()).hexdigest(), 'harness_sha256': {}, 'runs': {}, 'cleanup_errors': []}
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-oracle-build-') as tmp:
            context = pathlib.Path(tmp)
            for path in HERE.iterdir():
                if path.name.endswith('_test.go') or path.name == 'Capture.Dockerfile':
                    data = path.read_bytes()
                    (context / path.name).write_bytes(data)
                    report['harness_sha256'][path.name] = hashlib.sha256(data).hexdigest()
            with (args.output / 'build.log').open('wb') as log:
                run(['docker', 'build', '--tag', tag, '--file', str(context / 'Capture.Dockerfile'), str(context)], 1800, stdout=log, stderr=subprocess.STDOUT)
        report['image_id'] = subprocess.check_output(['docker', 'image', 'inspect', '--format', '{{.Id}}', tag], timeout=30).decode().strip()
        for component in ('first', 'repeat'):
            name = tag + '-' + component
            containers.append(name)
            # Finite suite of trusted source tests; Docker caps file output and memory.
            command = ['docker', 'run', '--name', name, '--network', 'none', '--read-only', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges', '--pids-limit', '128', '--memory', '512m', '--cpus', '2', '--ulimit', 'fsize=1048576:1048576', '--tmpfs', '/tmp:rw,nosuid,nodev,size=64m', tag, '/out/config.test', '-test.run', '^TestRubixCapture$', '-test.v', '-test.timeout', '90s']
            with (args.output / (component + '.log')).open('wb') as log:
                run(command, 110, stdout=log, stderr=subprocess.STDOUT)
            raw = (args.output / (component + '.log')).read_bytes()
            if len(raw) > 1048576:
                raise ValueError('capture exceeds 1MiB')
            records = verify.raw_records(args.output / (component + '.log'))
            verify.verify(records)
            (args.output / (component + '.json')).write_text(json.dumps(records, indent=2, sort_keys=True) + '\n')
            report['runs'][component] = {'exit_code': 0, 'stdout_sha256': hashlib.sha256(raw).hexdigest()}
        if (args.output / 'first.json').read_bytes() != (args.output / 'repeat.json').read_bytes():
            raise ValueError('repeat differs')
        report['repeat_equal'] = True
    except Exception as error:
        report['capture_error'] = str(error)
        raise
    finally:
        cleanup_and_receipt(report, tag, containers, args.output)
    if report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']:
        raise RuntimeError('owned resources remain')

if __name__ == '__main__':
    main()
