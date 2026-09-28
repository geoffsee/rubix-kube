#!/usr/bin/env python3
"""Actual pinned Go observations in disposable containers, never host sysctls."""
import argparse, hashlib, importlib.util, json, pathlib, shutil, tempfile, uuid
import verify
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('lifecycle',ROOT/'tools/defaults/capture.py')
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args()
 hashes={n:digest(HERE/n) for n in verify.INPUTS};helper_hash=digest(ROOT/'tools/defaults/capture.py')
 args.output.mkdir(parents=True,exist_ok=False);tag='rubix-constrained-'+uuid.uuid4().hex
 report={'revision':verify.REVISION,'archive_sha256':verify.ARCHIVE,'source_sha256':hashes,'helper_sha256':helper_hash,'containers':[],'errors':[],'cleanup_errors':[],'outputs':{}}
 try:
  with tempfile.TemporaryDirectory() as temporary:
   for name in verify.BUILD_INPUTS:shutil.copyfile(HERE/name,pathlib.Path(temporary)/name)
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,'-f',temporary+'/Capture.Dockerfile',temporary],args.output/'build.log',900,8*1024*1024)
  helper.bounded(['docker','image','inspect','--format','{{.Id}}',tag],args.output/'image.log',30,65536)
  report['image_id']=(args.output/'image.log').read_text().strip()
  expected=verify.load(HERE/'expected.json')
  for repeat in range(2):
   for family in verify.FAMILIES:
    name=tag+'-'+family+'-'+str(repeat);report['containers'].append(name);log=args.output/f'{family}{repeat}.log'
    helper.bounded(['docker','run','--name',name,'--network','none','--read-only','--cap-drop','ALL','--cap-add','SYS_CHROOT','--cap-add','SETUID','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=32m','--ulimit','fsize=1048576:1048576',tag,'/'+family+'.test','-test.run','^TestCapture$','-test.v','-test.timeout','30s'],log,45,1048576)
    verify.equal(verify.records(log),expected[family],'actual '+family)
    report['outputs'][log.name]=digest(log)
  helper.bounded(['docker','cp',report['containers'][0]+':/source.sha256',str(args.output/'source.sha256')],args.output/'copy.log',30,65536)
  report['outputs']['source.sha256']=digest(args.output/'source.sha256');report['identical_records']=True
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
