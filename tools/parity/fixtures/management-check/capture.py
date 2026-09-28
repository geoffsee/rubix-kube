#!/usr/bin/env python3
import verify
import argparse,hashlib,importlib.util,json,pathlib,shutil,tempfile,uuid
HERE=pathlib.Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('lifecycle',ROOT/'tools/defaults/capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
INPUTS=verify.INPUTS
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=pathlib.Path,required=True);args=parser.parse_args();hashes={n:digest(HERE/n) for n in INPUTS};args.output.mkdir(parents=True,exist_ok=False);tag='rubix-management-'+uuid.uuid4().hex
 report={'revision':'2ef1c4787989f11f868f81bb84ae2afd4a49a81d','archive_sha256':'9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec','source_sha256':hashes,'helper_sha256':digest(ROOT/'tools/defaults/capture.py'),'containers':[],'errors':[],'cleanup_errors':[],'outputs':{}}
 try:
  with tempfile.TemporaryDirectory() as temporary:
   for name in INPUTS:shutil.copyfile(HERE/name,pathlib.Path(temporary)/name)
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,'-f',temporary+'/Capture.Dockerfile',temporary],args.output/'build.log',900,8*1024*1024)
  helper.bounded(['docker','image','inspect','--format','{{.Id}}',tag],args.output/'image.log',30,65536);report['image_id']=(args.output/'image.log').read_text().strip()
  records=[]
  for repeat in range(2):
   name=tag+'-'+str(repeat);report['containers'].append(name);log=args.output/f'run{repeat}.log'
   helper.bounded(['docker','run','--name',name,'--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','256m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag,'/capture.test','-test.run','^TestCapture$','-test.v','-test.timeout','45s'],log,60,1048576)
   record=[verify.records(log)];
   if len(record)!=1:raise ValueError('record count')
   verify.equal([verify.classify(r) for r in record[0]],verify.load(HERE/'expected.json'),'independent expectations');records.append(record[0]);report['outputs'][log.name]=digest(log)
  if records[0]!=records[1]:raise ValueError('repeat mismatch')
  helper.bounded(['docker','cp',report['containers'][0]+':/source.sha256',str(args.output/'source.sha256')],args.output/'copy.log',30,65536);report['outputs']['source.sha256']=digest(args.output/'source.sha256');report['identical_records']=True
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
