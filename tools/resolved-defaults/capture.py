#!/usr/bin/env python3
"""Explicit official Complete() extraction in owned disposable containers."""
import argparse, hashlib, importlib.util, json, pathlib, shutil, subprocess, tempfile, uuid
HERE=pathlib.Path(__file__).resolve().parent
HELPER=HERE.parent/'defaults/capture.py'
spec=importlib.util.spec_from_file_location('defaults_capture',HELPER)
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--source-archive',type=pathlib.Path,required=True)
 parser.add_argument('--go-archive',type=pathlib.Path,required=True)
 parser.add_argument('--output',type=pathlib.Path,required=True)
 args=parser.parse_args();inputs=json.loads((HERE/'inputs.json').read_text())
 for name,path in [('source',args.source_archive),('go',args.go_archive)]:
  if path.stat().st_size!=inputs[name]['bytes'] or digest(path)!=inputs[name]['sha256']:raise ValueError(name+' input identity mismatch')
 args.output.mkdir(parents=True,exist_ok=False);tag='rubix-resolved-'+uuid.uuid4().hex
 report={'inputs':inputs,'source_sha256':{n:digest(HERE/n) for n in ('main.go','Dockerfile','capture.py','inputs.json')},'helper_sha256':digest(HELPER),'containers':[],'errors':[],'cleanup_errors':[]}
 try:
  with tempfile.TemporaryDirectory(prefix='rubix-resolved-build-') as d:
   work=pathlib.Path(d)
   for source,name in [(args.source_archive,'source.tar.gz'),(args.go_archive,'go.tar.gz')]:shutil.copyfile(source,work/name)
   for name in ('main.go','Dockerfile'):shutil.copyfile(HERE/name,work/name)
   helper.bounded(['docker','build','--network','none','--platform','linux/arm64','--tag',tag,'--build-arg','SOURCE_SHA256='+inputs['source']['sha256'],str(work)],args.output/'build.log',1800)
  report['image_inspect']=json.loads(subprocess.check_output(['docker','image','inspect',tag],timeout=30))
  records=[]
  for index in range(2):
   name=tag+'-'+str(index);report['containers'].append(name)
   log=args.output/f'run{index}.log'
   helper.bounded(['docker','run','--name',name,'--hostname','fixture-'+str(index),'--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','512m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag],log,90,2*1024*1024)
   lines=[line.removeprefix('RUBIX_RESOLVED=') for line in log.read_text().splitlines() if line.startswith('RUBIX_RESOLVED=')]
   if len(lines)!=1:raise ValueError('missing or duplicate capture record')
   records.append(lines[0]);(args.output/f'run{index}.json').write_text(json.dumps(json.loads(lines[0]),indent=2,sort_keys=True)+'\n')
  if records[0]!=records[1]:raise ValueError('completion was nondeterministic')
  report['identical_repeats']=True
  report['output_sha256']={f'run{i}.json':digest(args.output/f'run{i}.json') for i in range(2)}
  subprocess.run(['docker','cp',report['containers'][0]+':/out/modules.sha256',str(args.output/'modules.sha256')],check=True,timeout=30)
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
