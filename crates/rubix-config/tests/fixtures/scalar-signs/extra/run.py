import importlib.util,pathlib,uuid,json
root=pathlib.Path('/Volumes/safe-vol/workspace/archives/playground/rubix-kube');spec=importlib.util.spec_from_file_location('helper',root/'tools/defaults/capture.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
here=pathlib.Path(__file__).parent;out=here/'evidence';out.mkdir();tag='rubix-scalar-review-'+uuid.uuid4().hex;report={'containers':[],'errors':[],'cleanup_errors':[]}
try:
 h.bounded(['docker','build','--platform','linux/arm64','-t',tag,str(here)],out/'build.log',1800)
 name=tag+'-run';report['containers'].append(name)
 h.bounded(['docker','run','--name',name,'--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','512m','--cpus','2','--pids-limit','128','--tmpfs','/tmp:rw,nosuid,nodev,size=64m',tag,'/out/scalar.test','-test.run','^TestRubixCapture$','-test.v','-test.timeout','60s'],out/'run.log',90,1048576)
except Exception as e:report['errors'].append(str(e))
finally:h.finish(report,out,tag)
if report['errors'] or report['cleanup_errors']:raise SystemExit(1)
