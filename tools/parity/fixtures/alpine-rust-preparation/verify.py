"""Read-only actual Rust/Alpine verification with independent expected effects."""
import importlib.util,json,re,sys,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
spec=importlib.util.spec_from_file_location('alpine_baseline_verify',HERE.parent/'alpine-preparation/verify.py');baseline=importlib.util.module_from_spec(spec);spec.loader.exec_module(baseline)
require=baseline.require;read=baseline.read;digest=baseline.digest;loads=baseline.loads;equal=baseline.equal
WARNING=baseline.WARNING
SOURCES={'capture.py','inputs.json','guest.sh','reboot.sh','verify.py','service-double.sh'}
def cases(text):
 pattern=r'CASE_([a-z_]+)_BEGIN\nEXIT ([0-9]+)\nSTDOUT_BEGIN\n(.*?)STDOUT_END\nSTDERR_BEGIN\n(.*?)STDERR_END\nCASE_\1_END'
 rows=re.findall(pattern,text,re.S)
 require(2*len(rows)==text.count('CASE_'),'complete case frames')
 return [{'name':n,'exit':int(code),'stdout':out,'stderr':err} for n,code,out,err in rows]
def semantic(before,reboot):
 rows=cases(before)+cases(reboot)
 equal([r['name'] for r in rows],['opt_out','package_failure','service_failure','injected_no_effect_service','prepare','repeat','reboot'],'actual CLI case order')
 equal([r['exit'] for r in rows],[1,1,1,1,0,0,0],'actual CLI exit classification')
 for row in rows:
  require(row['stdout']=='','CLI stdout')
  require(('All 7 checks passed' in row['stderr'])==(row['exit']==0),'no false success')
  if row['name'] in ['package_failure','service_failure']:
   require('prerequisite preparation command failed or exceeded its deadline' in row['stderr'] and 'host state may have changed' in row['stderr'],'action failure and partial effects')
 plans=[[line.strip() for line in r['stderr'].splitlines() if '> Preparing' in line] for r in rows]
 equal(plans,[[],['> Preparing Alpine networking packages'],['> Preparing Alpine networking packages','> Preparing Alpine cgroups service'],['> Preparing Alpine cgroups service'],['> Preparing Alpine cgroups service'],[],[]],'ordered plans and idempotence')
 require('prerequisite is still missing after preparation' in rows[3]['stderr'],'no-effect service rejected after actual reobservation')
 hashes={name:re.findall(r'^SERVICE_'+name+r' ([0-9a-f]{64})  /etc/init.d/cgroups$',before,re.M) for name in ['ORIGINAL','DOUBLE','RESTORED']}
 require(all(len(values)==1 for values in hashes.values()),'service byte identity markers')
 require(hashes['ORIGINAL']==hashes['RESTORED'] and hashes['ORIGINAL']!=hashes['DOUBLE'],'actual service restored')
 require(hashes['ORIGINAL']==[loads(read(HERE/'inputs.json'))['openrc_service_sha256']],'pinned official OpenRC service bytes')
 require(hashes['DOUBLE']==[digest(HERE/'service-double.sh')],'explicit no-effect fixture source')
 require('required Alpine networking tools are missing' in rows[0]['stderr'],'actual opt-out blocker')
 for marker in ['NO_OPT_IN_UNCHANGED','PACKAGE_FAILURE_UNCHANGED','RUST_PREPARATION_COMPLETE']:require(before.splitlines().count(marker)==1,'state evidence '+marker)
 require(reboot.splitlines().count('RUST_REBOOT_COMPLETE')==1,'reboot completion')
 before_packages=baseline.package_section(before,'before');installed=baseline.package_section(before,'installed')
 require(installed-before_packages==baseline.ADDED and not before_packages-installed,'same baseline exact eight-package delta')
 require(baseline.package_section(before,'after_service_failure')==installed,'partial package effects retained after real registration failure')
 require(baseline.package_section(reboot,'reboot')==installed,'reboot installed inventory')
 require('CONTROLLERS_before ABSENT' in before.splitlines(),'naturally absent controllers')
 for name,text in [('after',before),('repeated',before),('reboot',reboot)]:require(text.splitlines().count('CONTROLLERS_'+name+' cpuset cpu io memory hugetlb pids dmem')==1,'actual controller state')
 original=baseline.verify()
 equal(sorted(before_packages),original['initial'],'baseline initial inventory')
 equal(sorted(installed),original['installed'],'baseline installed inventory')
 return {'cases':rows,'initial':sorted(before_packages),'installed':sorted(installed)}
def verify_public_key(value):
 require(type(value) is str and re.fullmatch(r'ssh-ed25519 [A-Za-z0-9+/=]+ rubix-alpine-fixture-host',value) is not None,'fixture-only public host key comment')
def verify_bootstrap(report,directory):
 for label,key in [('cloud-init','cloud_init'),('reboot-cloud-init','reboot_cloud_init')]:
  cloud=report[key];equal(cloud,loads(read(directory/(label+'.stdout'))),'raw '+label+' observation')
  code=report[key+'_exit'];require(type(code) is int and code in [0,2],'cloud exit classification')
  require(cloud.get('status')=='done' and cloud.get('errors')==[],'cloud bootstrap completion')
  warning={'WARNING':[WARNING]};warnings=cloud.get('recoverable_errors')
  require(warnings in ({},warning) and (code!=2 or warnings==warning),'only exact allowed cloud warning')
  for stage in ['init-local','init','modules-config','modules-final']:require(cloud[stage]['errors']==[],'cloud stage error')
def verify_guest(directory,here=HERE):
 report=loads(read(directory/'result.json'))
 require(report.get('status')=='passed' and report.get('errors')==[],'successful guest receipt')
 for key in ['owned_process_group_absent','owned_temporary_directory_removed','explicit_privilege_opt_in']:
  require(report.get(key) is True,'owned cleanup '+key)
 require(type(report.get('qemu_exit_code')) is int and report['qemu_exit_code']==0,'QEMU exit')
 require(report.get('shutdown')=='guest-poweroff','orderly guest shutdown')
 require(report.get('privileged_mount_probe')=='passed','actual guest mount privilege')
 require(isinstance(report.get('revision'),str) and re.fullmatch('[0-9a-f]{40}',report['revision']),'source revision')
 require(report.get('working_tree_snapshot') is True,'source snapshot scope')
 require(set(report['source_sha256'])==SOURCES,'harness source inventory')
 for name,value in report['source_sha256'].items():require(digest(here/name)==value,'current source '+name)
 require(report['inherited_vm_sha256']==digest(ROOT/'tools/parity/vm/run.py'),'inherited lifecycle source')
 equal(report['inputs'],loads(read(here/'inputs.json')),'accepted inputs')
 equal({name:tool['sha256'] for name,tool in report['tools'].items()},report['inputs']['host_tools'],'exact host tool pins')
 equal(report['firmware'],report['inputs']['firmware'],'exact firmware pins')
 require(report['baseline_helper_sha256']==digest(HERE.parent/'alpine-preparation/capture.py'),'executed lifecycle helper')
 require(report['build_verifier_sha256']==digest(HERE.parent/'prerequisite-preparation/verify_linux.py'),'executed build verifier')
 require(report['build_support_sha256']==digest(HERE.parent/'prerequisite-preparation/support.py'),'executed build support')
 require(report['artifact_sha256']==report['inputs']['artifact']['sha256'],'executed binary pin')
 subprocess.run([sys.executable,str(HERE.parent/'prerequisite-preparation/verify_linux.py'),str(directory/'artifact-build')],check=True,timeout=30,stdout=subprocess.DEVNULL)
 metadata=loads(read(directory/'artifact-build/artifact.json'));receipt=loads(read(directory/'artifact-build/receipt.json'))
 for key in ['sha256','size','target','revision']:equal(metadata[key],report['inputs']['artifact'][key],'candidate '+key)
 equal(receipt['revision'],metadata['revision'],'candidate build revision')
 verify_bootstrap(report,directory)
 require(report.get('serial_log_truncated',False) is False,'complete bounded console')
 private=report['owned_temporary_directory'];require(private.startswith('/tmp/rubix-vm-'),'owned private prefix')
 port=report['ssh_forward'];require(re.fullmatch(r'127\.0\.0\.1:[0-9]+',port),'loopback SSH only')
 qemu=['qemu-system-aarch64','-machine','virt,accel=hvf','-cpu','host','-smp','2','-m','2048',
  '-display','none','-serial','stdio','-monitor','none','-qmp','unix:'+private+'/qmp.sock,server=on,wait=off',
  '-drive','if=pflash,format=raw,readonly=on,file=/opt/homebrew/share/qemu/edk2-aarch64-code.fd',
  '-drive','if=pflash,format=raw,file='+private+'/vars.fd','-drive','if=virtio,format=qcow2,file='+private+'/disk.qcow2',
  '-drive','if=virtio,format=raw,readonly=on,file='+private+'/seed.iso','-netdev',
  'user,id=n0,restrict=on,hostfwd=tcp:'+port+'-:22','-device','virtio-net-pci,netdev=n0']
 equal(report['qemu_argv'],qemu,'exact owned VM resources and restricted network')
 verify_public_key(report.get('guest_host_public_key',''))
 old=read(directory/'before-reboot-id.stdout').strip();new=read(directory/'reboot-readiness.stdout').strip()
 require(old!=new and all(re.fullmatch(rb'[0-9a-f-]{36}',value) for value in [old,new]),'actual fresh kernel boot')
 before=read(directory/'baseline-inventory.stdout').decode();equal(report['observation'],before,'raw observation binding')
 return semantic(before,read(directory/'reboot-verification.stdout').decode())
def verify(here=HERE):
 provenance=loads(read(here/'provenance.json'));require(set(provenance)=={'files'},'provenance shape')
 actual={str(p.relative_to(here/'evidence')) for p in (here/'evidence').rglob('*') if p.is_file()}
 require(actual==set(provenance['files']),'exact evidence inventory')
 for name,value in provenance['files'].items():
  require(not Path(name).is_absolute() and '..' not in Path(name).parts,'safe evidence path')
  raw=read(here/'evidence'/name);require(digest(here/'evidence'/name)==value,'raw evidence digest')
  require(b'BEGIN OPENSSH PRIVATE KEY' not in raw,'no private credentials')
 first=verify_guest(here/'evidence/first',here);repeat=verify_guest(here/'evidence/repeat',here)
 equal(first,repeat,'independent repeated Rust guest observations')
 return first
if __name__=='__main__':verify();print('Rust Alpine preparation evidence verified')
