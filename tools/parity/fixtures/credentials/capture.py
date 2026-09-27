#!/usr/bin/env python3
"""Capture pinned baseline credential behavior in disposable containers."""
import argparse, hashlib, importlib.util, json, pathlib, shutil, subprocess, tempfile, uuid
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('bounded_defaults',ROOT/'tools/defaults/capture.py')
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
 tag='rubix-credentials-'+uuid.uuid4().hex
 report={'revision':'2ef1c4787989f11f868f81bb84ae2afd4a49a81d','source_archive_sha256':'9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','builder':'golang:1.26.5-bookworm@sha256:53eeac89074db483fdf0ab3be1df32bf6e47562263d2d0d6baa7f26acb4957dd','source_sha256':{p.name:digest(p) for p in HERE.iterdir() if p.suffix=='.go' or p.name in ('Capture.Dockerfile','capture.py')},'helper_sha256':digest(ROOT/'tools/defaults/capture.py'),'containers':[],'errors':[],'cleanup_errors':[],'outputs':{}}
 try:
  with tempfile.TemporaryDirectory(prefix='rubix-credentials-build-') as temporary:
   context=pathlib.Path(temporary)
   for name in ('apiserver_capture_test.go','kubelet_capture_test.go','Capture.Dockerfile'):shutil.copyfile(HERE/name,context/name)
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,'-f',str(context/'Capture.Dockerfile'),str(context)],args.output/'build.log',1800)
  report['image_id']=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',tag],timeout=30).decode().strip()
  for component in ('apiserver','kubelet'):
   records=[]
   for repeat in range(2):
    name=tag+'-'+component+'-'+str(repeat);report['containers'].append(name)
    log=args.output/(component+'-'+str(repeat)+'.log')
    helper.bounded(['docker','run','--name',name,'--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','512m','--cpus','2','--pids-limit','128','--tmpfs','/tmp:rw,nosuid,nodev,size=64m','--ulimit','fsize=1048576:1048576',tag,'/out/'+component+'.test','-test.run','^TestRubixCapture$','-test.v','-test.timeout','90s'],log,110,1048576)
    values=[json.loads(line.removeprefix('RUBIX_CAPTURE ')) for line in log.read_text().splitlines() if line.startswith('RUBIX_CAPTURE ')]
    if len(values)!=1:raise ValueError('expected exactly one record')
    records.append(values[0])
   if records[0]!=records[1]:raise ValueError('nondeterministic public output')
   path=args.output/(component+'.json');path.write_text(json.dumps(records[0],indent=2,sort_keys=True)+'\n');report['outputs'][path.name]=digest(path)
  report['identical_repeats']=True
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 if report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']:raise SystemExit(1)
if __name__=='__main__':main()
