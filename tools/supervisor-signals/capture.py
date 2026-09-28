#!/usr/bin/env python3
"""Qualify only owned signal-fixture subprocesses in a disposable Linux container."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
spec=importlib.util.spec_from_file_location('bounded_defaults',ROOT/'tools/defaults/capture.py')
helper=importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
    tag='rubix-signal-capture-'+uuid.uuid4().hex
    report={'schema_version':1,'platform':'linux/arm64','scope':'cooperative task signals and owned process adapter signals; no cluster or escaped-daemon qualification','source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True,timeout=30).strip(),'uncommitted_implementation':True,'builder':'rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97','containers':[],'errors':[],'cleanup_errors':[],'source_sha256':{},'helper_sha256':digest(ROOT/'tools/defaults/capture.py')}
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-signal-build-') as temporary:
            context=Path(temporary)
            for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:shutil.copyfile(ROOT/name,context/name)
            for name in ['.cargo','crates','third_party','tools/upstream','tools/supervisor-process','tools/supervisor-signals']:
                shutil.copytree(ROOT/name,context/name,ignore=shutil.ignore_patterns('target','__pycache__','.DS_Store','evidence'))
            shutil.copyfile(HERE/'Capture.Dockerfile',context/'Dockerfile')
            inventory={str(path.relative_to(context)):digest(path) for path in sorted(context.rglob('*')) if path.is_file()}
            (args.output/'source-hashes.json').write_text(json.dumps(inventory,indent=2,sort_keys=True)+'\n')
            report['source_sha256']={'capture.py':digest(HERE/'capture.py'),'verify.py':digest(HERE/'verify.py'),'Capture.Dockerfile':digest(HERE/'Capture.Dockerfile'),'source-hashes.json':digest(args.output/'source-hashes.json')}
            helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,str(context)],args.output/'build.log',1800,16*1024*1024)
        report['image_id']=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',tag],timeout=30).decode().strip()
        name=tag+'-test';report['containers'].append(name)
        helper.bounded(['docker','run','--name',name,'--init','--env','RUBIX_SIGNAL_DISPOSABLE=1','--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','512m','--cpus','2','--pids-limit','128','--tmpfs','/tmp:rw,nosuid,nodev,size=16m','--ulimit','fsize=1048576:1048576',tag,'sh','-c','cat /out/signals.sha256 /out/owned_signals.sha256; /out/signals.test --nocapture && /out/owned_signals.test --ignored --exact disposable_owned_signals --nocapture ; status=$?; /usr/local/bin/python3 /namespace_inventory.py; inventory=$?; test "$status" -eq 0 && test "$inventory" -eq 0'],args.output/'run.log',180,1024*1024)
        lines=(args.output/'run.log').read_text().splitlines()
        expected='RUBIX_QUALIFICATION repetitions=20 full_partial_signal_cases=160 worker_failure_cases=20 all_children_reaped=true'
        if lines.count(expected)!=1:raise ValueError('missing exact completed qualification record')
        if not any(line.startswith('test result: ok. 1 passed; 0 failed; 1 ignored;') for line in lines):raise ValueError('unexpected test result')
        binaries=[line.split()[0] for line in lines if line.endswith('  /out/signals.test')]
        if len(binaries)!=1 or len(binaries[0])!=64 or any(c not in '0123456789abcdef' for c in binaries[0]):raise ValueError('invalid test binary hash')
        owned=[line.split()[0] for line in lines if line.endswith('  /out/owned_signals.test')]
        if len(owned)!=1 or len(owned[0])!=64:raise ValueError('owned binary hash')
        import verify
        verify.validate_owned(lines)
        report.update(owned_binary_sha256=owned[0],combined_cases=61,test_binary_sha256=binaries[0],repetitions=20,signal_cases=160,worker_failure_cases=20,owned_subprocesses_reaped=True,run_sha256=digest(args.output/'run.log'))
        # Fail if any source copied into the binary changes before evidence publication.
        for name,expected_hash in inventory.items():
            if name!='Dockerfile' and digest(ROOT/name)!=expected_hash:raise ValueError('source changed during qualification: '+name)
    except Exception as error:
        report['errors'].append(str(error))
    finally:
        helper.finish(report,args.output,tag)
    return 1 if report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images'] else 0

if __name__=='__main__':raise SystemExit(main())
