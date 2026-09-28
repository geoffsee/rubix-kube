#!/usr/bin/env python3
"""Capture unchanged baseline detection inside owned, filesystem-isolated children."""
import argparse,hashlib,importlib.util,json,pathlib,shutil,subprocess,tempfile,uuid
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('lifecycle',ROOT/'tools/defaults/capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
 tag='rubix-platform-capture-'+uuid.uuid4().hex
 report={'revision':'2ef1c4787989f11f868f81bb84ae2afd4a49a81d','archive_sha256':'9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','source_sha256':{n:digest(HERE/n) for n in ['capture.py','Capture.Dockerfile','detect_capture_test.go','expected.tsv']},'helper_sha256':digest(ROOT/'tools/defaults/capture.py'),'containers':[],'errors':[],'cleanup_errors':[],'outputs':{}}
 try:
  with tempfile.TemporaryDirectory() as temporary:
   for name in ['Capture.Dockerfile','detect_capture_test.go']:shutil.copyfile(HERE/name,pathlib.Path(temporary)/name)
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,'-f',temporary+'/Capture.Dockerfile',temporary],args.output/'build.log',900,8*1024*1024)
  report['image_id']=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',tag],timeout=30).decode().strip()
  expected=(HERE/'expected.tsv').read_text().splitlines()
  for repeat in range(2):
   name=tag+'-'+str(repeat);report['containers'].append(name);log=args.output/f'run{repeat}.log'
   helper.bounded(['docker','run','--name',name,'--network','none','--read-only','--cap-drop','ALL','--cap-add','SYS_CHROOT','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=16m','--ulimit','fsize=1048576:1048576',tag,'/detect.test','-test.run','^TestRubixCapture$','-test.v','-test.timeout','30s'],log,45,1048576)
   records=[json.loads(line.removeprefix('RUBIX_CAPTURE ')) for line in log.read_text().splitlines() if line.startswith('RUBIX_CAPTURE ')]
   if records!=[expected]:raise ValueError('baseline semantic mismatch')
   report['outputs'][log.name]=digest(log)
  subprocess.run(['docker','cp',report['containers'][0]+':/source.sha256',str(args.output/'source.sha256')],check=True,timeout=30)
  report['outputs']['source.sha256']=digest(args.output/'source.sha256');report['identical_records']=True
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
