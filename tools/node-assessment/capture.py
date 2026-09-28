#!/usr/bin/env python3
"""Qualify node assessment only in owned disposable Linux containers."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import uuid
import verify
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location('output_bounded', ROOT/'tools/defaults/capture.py')
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
def control(argv):
    with tempfile.TemporaryDirectory(prefix='rubix-node-assessment-control-') as tmp:
        path = Path(tmp)/'log'
        helper.bounded(argv, path, 30, 65536)
        return path.read_text().strip()
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    revision = control(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'])
    verify.require(__import__('re').fullmatch('[0-9a-f]{40}', revision) is not None, 'source revision')
    relevant = verify.current_inventory()
    dirty = bool(control(['git', '-C', str(ROOT), 'status', '--porcelain', '--untracked-files=all', '--', *sorted(relevant)]))
    hashes = {name: verify.digest(HERE/name) for name in verify.HARNESS}
    helper_hash = verify.digest(ROOT/'tools/defaults/capture.py')
    tag = 'rubix-node-assessment-'+uuid.uuid4().hex
    report = dict(schema=1, source_revision=revision, uncommitted_source_snapshot=dirty, tag=tag,
                  harness_sha256=hashes, helper_sha256=helper_hash, containers=[],
                  errors=[], cleanup_errors=[], runs={})
    args.output.mkdir(parents=True, exist_ok=False)
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-node-assessment-source-') as tmp:
            context = Path(tmp)
            for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']:
                shutil.copyfile(ROOT/name, context/name)
            for name in ['.cargo', 'crates', 'third_party', 'tools/upstream', 'tools/supervisor-process', 'tools/node-assessment']:
                shutil.copytree(ROOT/name, context/name, ignore=shutil.ignore_patterns('target', '__pycache__', 'evidence', '.DS_Store'))
            inventory = {str(path.relative_to(context)): verify.digest(path) for path in sorted(context.rglob('*')) if path.is_file()}
            (args.output/'source-inventory.json').write_text(json.dumps(inventory, sort_keys=True, indent=2)+'\n')
            report['source_inventory_sha256'] = verify.digest(args.output/'source-inventory.json')
            helper.bounded(['docker','build','--platform=linux/arm64','-t',tag,'-f',str(context/'tools/node-assessment/Capture.Dockerfile'),str(context)], args.output/'build.log',1800,16*1024*1024)
        report['image_id'] = control(['docker','image','inspect','--format','{{.Id}}',tag])
        for name in ['first','repeat']:
            container = tag+'-'+name
            report['containers'].append(container)
            command = verify.run_command(tag, name)
            helper.bounded(command,args.output/(name+'.log'),100,1024*1024)
            records, binaries = verify.records(args.output/(name+'.log'))
            report['runs'][name] = dict(command=command, raw_sha256=verify.digest(args.output/(name+'.log')), binary_sha256=binaries, records=records)
        for name, digest in inventory.items():
            if verify.digest(ROOT/name) != digest: raise ValueError('source changed during capture: '+name)
    except Exception as error:
        report['errors'].append(str(error))
    finally:
        helper.finish(report,args.output,tag)
    verify.capture(args.output)
    return 0
if __name__ == '__main__': raise SystemExit(main())
