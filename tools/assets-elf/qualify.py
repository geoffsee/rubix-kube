"""Explicit bounded download and read-only inspection of four already-pinned releases."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,tempfile,time,urllib.request
from oracle import inspect
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[1]
CAP=256*1024*1024

def digest(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
def sources():
    paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'rust-toolchain.toml',ROOT/'experiments/component-boundary/inputs.json',ROOT/'tools/defaults/capture.py']
    paths.extend(p for p in (ROOT/'.cargo').rglob('*') if p.is_file())
    paths.extend(p for p in (ROOT/'crates').rglob('Cargo.toml') if 'target' not in p.parts)
    for name in ['rubix-assets','rubix-platform']:
        paths.extend(p for p in (ROOT/'crates'/name).rglob('*') if p.is_file() and 'target' not in p.parts and p.suffix in ['.rs','.json','.bin'])
    paths.extend(HERE/name for name in ['qualify.py','oracle.py','verify.py','test_evidence.py'])
    return {str(p.relative_to(ROOT)):digest(p) for p in sorted(set(paths))}
def fetch(path,record):
    if path.is_symlink():raise ValueError('cache symlink')
    if path.exists():
        if not path.is_file() or path.stat().st_size>CAP or digest(path)!=record['sha256']:raise ValueError('cache identity')
        return
    started=time.monotonic();temporary=None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent,delete=False) as out:
            temporary=pathlib.Path(out.name);total=0
            with urllib.request.urlopen(record['url'],timeout=30) as response:
                while block:=response.read(65536):
                    total+=len(block)
                    if total>CAP or time.monotonic()-started>300:raise ValueError('download bound')
                    out.write(block)
        if digest(temporary)!=record['sha256']:raise ValueError('download identity')
        # A cache race cannot replace an existing caller entry.
        os.link(temporary,path)
    finally:
        if temporary is not None:temporary.unlink(missing_ok=True)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--cache',type=pathlib.Path,required=True);parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args()
    args.cache=args.cache.absolute();args.output=args.output.absolute()
    hashes=sources();revision=subprocess.run(['git','rev-parse','HEAD'],cwd=ROOT,check=True,capture_output=True,text=True,timeout=10).stdout.strip()
    if args.cache.is_symlink():raise ValueError('cache directory symlink')
    args.cache.mkdir(parents=True,exist_ok=True);args.output.mkdir(parents=True,exist_ok=False)
    report={'revision':revision,'working_tree_snapshot':True,'source_sha256':hashes,'artifacts':{},'errors':[],'qualification':'read-only ELF inspection; no artifact execution'}
    try:
        inputs=json.loads((ROOT/'experiments/component-boundary/inputs.json').read_text())['artifacts']
        for arch,roles in inputs.items():
            for role,record in roles.items():
                name=f'{role}-{arch}';path=args.cache/name;fetch(path,record)
                with path.open('rb') as stream:data=stream.read(CAP+1)
                if len(data)>CAP or hashlib.sha256(data).hexdigest()!=record['sha256']:raise ValueError('oracle byte identity')
                report['artifacts'][name]={'sha256':record['sha256'],'url':record['url'],'oracle':inspect(data)}
        spec=importlib.util.spec_from_file_location('bounded_capture',ROOT/'tools/defaults/capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
        previous=os.getcwd();os.chdir(ROOT)
        try:
            for index in range(2):
                helper.bounded(['env','RUBIX_ELF_FIXTURES='+str(args.cache.resolve()),'cargo','test','-p','rubix-assets','--release','--locked','--offline','--test','elf_real','--','--ignored','--nocapture'],args.output.resolve()/f'run{index}.log',900,8*1024*1024)
        finally:os.chdir(previous)
        if hashes!=sources():raise ValueError('source changed')
        for name,record in report['artifacts'].items():
            if digest(args.cache/name)!=record['sha256']:raise ValueError('artifact changed')
    except Exception as error:report['errors'].append(type(error).__name__+': '+str(error))
    finally:
        report['logs']={p.name:digest(p) for p in args.output.glob('run*.log')}
        (args.output/'receipt.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
    if report['errors']:raise SystemExit(1)
if __name__=='__main__':main()
