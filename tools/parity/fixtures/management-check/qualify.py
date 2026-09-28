"""Build exact Rust source and run only owned disposable management checks."""
import argparse,json,pathlib,re,shutil,tempfile,uuid
from capture import HERE,ROOT,helper,digest
import verify_linux

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args()
 helper_hash=digest(ROOT/'tools/defaults/capture.py');harness_hashes={n:digest(HERE/n) for n in verify_linux.HARNESS}
 with tempfile.TemporaryDirectory() as temporary:
  path=pathlib.Path(temporary)/'revision';helper.bounded(['git','-C',str(ROOT),'rev-parse','HEAD'],path,10,4096);revision=path.read_text().strip()
 if re.fullmatch('[a-f0-9]{40}',revision) is None:raise ValueError('source revision')
 args.output.mkdir(parents=True,exist_ok=False);tag='rubix-management-linux-'+uuid.uuid4().hex
 report={'revision':revision,'uncommitted_implementation':True,'platform':'linux/arm64','target':'aarch64-unknown-linux-musl','containers':[],'errors':[],'cleanup_errors':[],'helper_sha256':helper_hash,'source_sha256':harness_hashes}
 try:
  with tempfile.TemporaryDirectory() as temporary:
   context=pathlib.Path(temporary)
   for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:shutil.copyfile(ROOT/n,context/n)
   for n in ['crates','third_party','.cargo','tools/upstream']:shutil.copytree(ROOT/n,context/n,ignore=shutil.ignore_patterns('target','__pycache__'))
   target=context/'tools/parity/fixtures/management-check';target.mkdir(parents=True)
   for n in ['cases.json','expected.json','linux_fixture.py']:shutil.copyfile(HERE/n,target/n)
   shutil.copyfile(HERE/'Linux.Dockerfile',context/'Dockerfile')
   inventory={str(p.relative_to(context)):digest(p) for p in sorted(context.rglob('*')) if p.is_file()};(args.output/'source-hashes.json').write_text(json.dumps(inventory,sort_keys=True,indent=2)+'\n');report['inventory_sha256']=digest(args.output/'source-hashes.json')
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,str(context)],args.output/'build.log',900,8*1024*1024)
  helper.bounded(['docker','image','inspect','--format','{{.Id}}',tag],args.output/'image.log',30,65536);report['image_id']=(args.output/'image.log').read_text().strip()
  name=tag+'-test';report['containers'].append(name)
  command=['docker','run','--name',name,'--hostname','fixture','--init','--read-only','--network','none','--cap-drop','ALL','--cap-add','SYS_CHROOT','--cap-add','SETUID','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,exec,nosuid,nodev,size=32m',tag]
  report['command']=command;helper.bounded(command,args.output/'run.log',60,1048576)
  verify_linux.verify_run((args.output/'run.log').read_bytes());report['run_sha256']=digest(args.output/'run.log')
  for name,value in inventory.items():
   if name!='Dockerfile' and digest(ROOT/name)!=value:raise ValueError('source changed: '+name)
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
