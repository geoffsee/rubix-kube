"""Qualify trusted supplemental probes exclusively in fresh owned Docker namespaces."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
spec = importlib.util.spec_from_file_location('bounded_helper', ROOT/'tools/defaults/capture.py')
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def short(argv):
    with tempfile.TemporaryDirectory(prefix='rubix-probe-control-') as temporary:
        log = Path(temporary)/'output'
        helper.bounded(argv, log, 30, 65536)
        return log.read_text().strip()

def main():
    parser = argparse.ArgumentParser(); parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    # Prepare bounded metadata first so failure leaves the requested output reusable.
    revision = short(['git','-C',str(ROOT),'rev-parse','HEAD'])
    helper_sha256 = digest(ROOT/'tools/defaults/capture.py')
    source_sha256 = {name:digest(HERE/name) for name in ['capture.py','verify.py','Capture.Dockerfile']}
    input_paths = ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','crates','third_party','tools/upstream',
                   'tools/defaults/capture.py','tools/parity/fixtures/platform-discovery/expected.tsv',
                   'tools/parity/fixtures/preflight-policy/expected.tsv']
    input_paths += [str((HERE/name).relative_to(ROOT)) for name in source_sha256]
    dirty = bool(short(['git','-C',str(ROOT),'status','--porcelain','--untracked-files=all','--',*input_paths]))
    args.output.mkdir(parents=True, exist_ok=False)
    tag = 'rubix-preflight-probes-'+uuid.uuid4().hex
    report = {'schema_version':1, 'revision':revision, 'platform':'linux/arm64',
              'uncommitted_implementation':dirty, 'containers':[], 'errors':[],
              'cleanup_errors':[], 'remaining_containers':None, 'remaining_images':None,
              'source_sha256':source_sha256, 'runs':{}, 'image_id':None,
              'helper_sha256':helper_sha256}
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-preflight-probes-source-') as temporary:
            context = Path(temporary)
            for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']: shutil.copyfile(ROOT/name,context/name)
            for name in ['crates','third_party','.cargo','tools/upstream']:
                shutil.copytree(ROOT/name,context/name,ignore=shutil.ignore_patterns('target','__pycache__'))
            for name in ['platform-discovery','preflight-policy']:
                path = Path('tools/parity/fixtures')/name/'expected.tsv'
                (context/path).parent.mkdir(parents=True); shutil.copyfile(ROOT/path,context/path)
            shutil.copyfile(HERE/'Capture.Dockerfile',context/'Dockerfile')
            inventory = {str(path.relative_to(context)):digest(path) for path in sorted(context.rglob('*')) if path.is_file()}
            (args.output/'source-hashes.json').write_text(json.dumps(inventory,indent=2,sort_keys=True)+'\n')
            report['inventory_sha256'] = digest(args.output/'source-hashes.json')
            helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,str(context)],args.output/'build.log',900,8*1024*1024)
        report['image_id'] = short(['docker','image','inspect','--format','{{.Id}}',tag])
        for index in range(2):
            name = tag+'-'+str(index); report['containers'].append(name)
            log = args.output/f'run{index}.log'
            helper.bounded(['docker','run','--name',name,'--read-only','--network','none','--cap-drop','ALL',
                            '--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64',
                            '--tmpfs','/tmp:rw,nosuid,nodev,size=16m','--env','RUBIX_PREFLIGHT_DISPOSABLE=1',tag],log,60,1024*1024)
            report['runs'][log.name] = digest(log)
        for name,value in inventory.items():
            if name != 'Dockerfile' and digest(ROOT/name) != value: raise ValueError('source changed during qualification: '+name)
    except Exception as error:
        report['errors'].append(str(error))
    finally:
        def cleanup(label, command):
            try: return short(command).splitlines()
            except Exception as error:
                report['cleanup_errors'].append(label+': '+str(error)); return None
        for name in report['containers']: cleanup('remove container', ['docker','rm','--force',name])
        cleanup('remove image',['docker','image','rm','--force',tag])
        report['remaining_containers'] = cleanup('inspect containers',['docker','ps','-aq','--filter','name='+tag])
        report['remaining_images'] = cleanup('inspect images',['docker','images','-q',tag])
        (args.output/'receipt.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
    return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))

if __name__ == '__main__': raise SystemExit(main())
