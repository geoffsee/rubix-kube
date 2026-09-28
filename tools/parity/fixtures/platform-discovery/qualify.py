"""Compile Linux-only code and exercise read-only discovery in a disposable container."""
import argparse,json,pathlib,shutil,subprocess,tempfile,uuid
from capture import ROOT,HERE,helper,digest

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
 tag='rubix-platform-linux-'+uuid.uuid4().hex
 report={'containers':[],'errors':[],'cleanup_errors':[],'source_sha256':{},'helper_sha256':digest(ROOT/'tools/defaults/capture.py')}
 try:
  report['revision']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,timeout=30,text=True).strip()
  report['uncommitted_implementation']=True
  report['platform']='linux/arm64'
  with tempfile.TemporaryDirectory() as temporary:
   context=pathlib.Path(temporary)
   for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:shutil.copyfile(ROOT/name,context/name)
   for name in ['crates','third_party','.cargo','tools/upstream']:shutil.copytree(ROOT/name,context/name,ignore=shutil.ignore_patterns('target','__pycache__'))
   target=context/'tools/parity/fixtures/platform-discovery';target.mkdir(parents=True);shutil.copyfile(HERE/'expected.tsv',target/'expected.tsv');shutil.copyfile(HERE/'Linux.Dockerfile',context/'Dockerfile')
   preflight=context/'tools/parity/fixtures/preflight-policy';preflight.mkdir(parents=True);shutil.copyfile(ROOT/'tools/parity/fixtures/preflight-policy/expected.tsv',preflight/'expected.tsv')
   inventory={str(p.relative_to(context)):digest(p) for p in sorted(context.rglob('*')) if p.is_file()};(args.output/'source-hashes.json').write_text(json.dumps(inventory,indent=2,sort_keys=True)+'\n')
   report['source_sha256']={name:digest(HERE/name) for name in ['qualify.py','capture.py','Linux.Dockerfile','expected.tsv']};report['inventory_sha256']=digest(args.output/'source-hashes.json')
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,str(context)],args.output/'build.log',900,8*1024*1024)
  report['image_id']=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',tag],timeout=30,text=True).strip()
  name=tag+'-test';report['containers'].append(name)
  helper.bounded(['docker','run','--name',name,'--read-only','--network','none','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag],args.output/'run.log',60,1048576)
  log=(args.output/'run.log').read_text()
  if 'test result: ok. 6 passed;' not in log or 'test result: ok. 1 passed;' not in log or 'test result: ok. 8 passed;' not in log or 'real_discovery_preserves_custom_paths_and_does_not_create_or_rewrite_them ... ok' not in log:raise ValueError('missing test completion')
  report['run_sha256']=digest(args.output/'run.log')
  for name,value in inventory.items():
   if name!='Dockerfile' and digest(ROOT/name)!=value:raise ValueError('source changed during capture: '+name)
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
