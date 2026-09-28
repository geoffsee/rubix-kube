"""Prepare public pinned packages and a static actual-baseline oracle; never boot a VM."""
import argparse,hashlib,importlib.util,json,pathlib,re,shutil,subprocess,tempfile,urllib.request,uuid
HERE=pathlib.Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('bounded_helper',ROOT/'tools/defaults/capture.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cache',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);args=p.parse_args()
 args.output.mkdir(parents=True,exist_ok=False);args.cache.mkdir(exist_ok=True)
 tag='rubix-alpine-prepare-'+uuid.uuid4().hex
 report={'helper_sha256':digest(ROOT/'tools/defaults/capture.py'),'containers':[],'errors':[],'cleanup_errors':[],'source_sha256':{n:digest(HERE/n) for n in ['Prepare.Dockerfile','go.mod','baseline_test.go','prepare.py']}}
 def short(label,argv):
  log=args.output/(label+'.log');helper.bounded(argv,log,30,65536);return log.read_text().strip()
 try:
  index=args.cache/'APKINDEX.tar.gz';expected='c9bc75e1d555694bcbdd2bfe53d733661a06c806291f460cd59c3f9fe648616a'
  if index.is_symlink() or digest(index)!=expected:raise ValueError('index pin')
  with tempfile.TemporaryDirectory(prefix='rubix-alpine-prepare-source-') as tmp:
   context=pathlib.Path(tmp)
   for n in ['Prepare.Dockerfile','go.mod','baseline_test.go']:shutil.copyfile(HERE/n,context/n)
   shutil.copyfile(index,context/'APKINDEX.tar.gz')
   helper.bounded(['docker','build','--platform','linux/arm64','-t',tag,'-f',str(context/'Prepare.Dockerfile'),str(context)],args.output/'build.log',900,8*1024*1024)
  name=tag+'-resolve';report['containers'].append(name)
  helper.bounded(['docker','run','--name',name,'--read-only','--network','none','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','256m','--pids-limit','64','--tmpfs','/tmp:rw,size=16m','--tmpfs','/var/cache/apk:rw,size=16m',tag,'apk','fetch','--simulate','--recursive','--url','--no-network','--repositories-file','/repositories','nftables=1.1.6-r1','iptables=1.8.13-r0','openrc=0.63.2-r0'],args.output/'resolution.log',60,65536)
  for source,dest in [('/preflight.test','preflight.test'),('/source.sha256','source.sha256')]:
   short('copy-'+dest,['docker','cp',name+':'+source,str(args.cache/dest)])
  names=[]
  for line in (args.output/'resolution.log').read_text().splitlines():
   if line.startswith('/repo/aarch64/') or line.startswith('file:///repo/aarch64/'):
    name=line.rsplit('/',1)[1]
    if not re.fullmatch(r'[A-Za-z0-9+_.-]+\.apk',name):raise ValueError('unsafe package filename')
    names.append(name)
  if not names or len(names)!=len(set(names)) or len(names)>64:raise ValueError('package inventory')
  accepted=json.loads((HERE/'inputs.json').read_text())['packages']['selected']
  if set(names)!=set(accepted):raise ValueError('package resolution drift')
  packages=args.cache/'packages';packages.mkdir(exist_ok=True)
  shutil.copyfile(index,packages/'APKINDEX.tar.gz')
  pins={}
  for name in names:
   dest=packages/name
   if dest.is_symlink():raise ValueError('symlink cache')
   url='https://dl-cdn.alpinelinux.org/alpine/v3.24/main/aarch64/'+name
   with urllib.request.urlopen(url,timeout=30) as response:data=response.read(16*1024*1024+1)
   if len(data)>16*1024*1024:raise ValueError('package size budget')
   if hashlib.sha256(data).hexdigest()!=accepted[name]['sha256']:raise ValueError('package digest drift')
   dest.write_bytes(data);pins[name]={'url':url,'sha256':digest(dest),'bytes':len(data)}
  report['packages']=pins;report['index_sha256']=expected;report['oracle_sha256']=digest(args.cache/'preflight.test');report['baseline_source_and_binary']=(args.cache/'source.sha256').read_text()
  (args.cache/'package-pins.json').write_text(json.dumps(pins,indent=2,sort_keys=True)+'\n')
 except Exception as error:report['errors'].append(str(error))
 finally:helper.finish(report,args.output,tag)
 return int(bool(report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images']))
if __name__=='__main__':raise SystemExit(main())
