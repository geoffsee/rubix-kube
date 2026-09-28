"""Strict source-bound observations from owned Alpine guests; no VM is started here."""
import hashlib,json,math,re
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
NETWORK_ERROR='required Alpine networking packages not found: nftables, iptables. kube-proxy needs both nftables (nft) and iptables (iptables-nft wrapper). Run the installer with --install-prereqs to install them automatically, or run: apk add nftables iptables'
CGROUP_ERROR='cgroup controllers are not available on this Alpine system. Run: rc-update add cgroups boot && rc-service cgroups start — or re-run the installer with --install-prereqs'
ADDED={'gmp-6.3.0-r4','iptables-1.8.13-r0','iptables-openrc-1.8.13-r0','jansson-2.15.0-r0','libnftnl-1.3.1-r0','libxtables-1.8.13-r0','nftables-1.1.6-r1','nftables-openrc-1.1.6-r1'}
SOURCES={'capture.py','inputs.json','guest.sh','reboot.sh','baseline_test.go','go.mod','Prepare.Dockerfile','prepare.py','verify.py'}
WARNING='Unable to activate module keys_to_console, helper tool not found at /usr/lib/cloud-init/write-ssh-key-fingerprints'
def require(ok,message):
 if not ok:raise ValueError(message)
def read(path):
 require(path.is_file() and not path.is_symlink() and path.stat().st_size<=8*1024*1024,'bounded regular evidence file');return path.read_bytes()
def digest(path):return hashlib.sha256(read(path)).hexdigest()
def loads(raw):
 def pairs(values):
  result={}
  for key,value in values:require(key not in result,'duplicate JSON key');result[key]=value
  return result
 def invalid(_):raise ValueError('nonfinite JSON')
 def finite(value):
  result=float(value);require(math.isfinite(result),'nonfinite JSON');return result
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=invalid,parse_float=finite)
def equal(a,b,message):require(json.dumps(a,sort_keys=True)==json.dumps(b,sort_keys=True),message)
def records(text):return [loads(line.removeprefix('RUBIX_ALPINE_BASELINE ')) for line in text.splitlines() if line.startswith('RUBIX_ALPINE_BASELINE ')]
def package_section(text,name):
 start='PACKAGES_'+name+'_BEGIN\n';end='PACKAGES_'+name+'_END'
 require(text.count(start)==1 and text.count(end)==1,'package inventory markers')
 lines=text.split(start)[1].split(end)[0].splitlines()
 require(lines==sorted(set(lines)) and len(lines)>100,'unique sorted complete installed inventory')
 return set(lines)
def semantic(before,reboot):
 expected=[{'action':'network','install':False,'error':NETWORK_ERROR},{'action':'cgroups','install':False,'error':CGROUP_ERROR}]
 expected += [{'action':action,'install':True,'error':''} for action in ['network','network','cgroups','cgroups']]
 equal(records(before),expected,'actual baseline call sequence/results')
 equal(records(reboot),[{'action':'cgroups','install':False,'error':''}],'reboot baseline check')
 for marker in ['RUBIX_ALPINE_BOOT_COMPLETE','RUBIX_NO_OPT_IN_UNCHANGED','RUBIX_PACKAGE_REPEAT_UNCHANGED','RUBIX_PREPARATION_COMPLETE']:
  require(before.splitlines().count(marker)==1,'completion '+marker)
 require(reboot.splitlines().count('RUBIX_REBOOT_COMPLETE')==1,'reboot completion')
 require(before.splitlines().count('CONTROLLERS_before ABSENT')==1,'actual missing controller precondition')
 controllers='cpuset cpu io memory hugetlb pids dmem'
 for name,text in [('after',before),('repeated',before),('reboot',reboot)]:
  require(text.splitlines().count('CONTROLLERS_'+name+' '+controllers)==1,'actual controllers '+name)
 initial=package_section(before,'before');installed=package_section(before,'installed')
 require(installed-initial==ADDED and not initial-installed,'exact installed package delta')
 require(package_section(reboot,'reboot')==installed,'reboot package inventory')
 require({'openrc-0.63.2-r0','apk-tools-3.0.8-r0','linux-virt-6.18.52-r0','alpine-release-3.24.2-r0'}<=initial,'guest system version pins')
 return {'initial':sorted(initial),'installed':sorted(installed),'records':expected,'controllers':controllers}
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
 cloud=report['cloud_init'];require(cloud.get('status')=='done' and cloud.get('errors')==[],'cloud bootstrap completion')
 require(cloud.get('recoverable_errors') in ({},{'WARNING':[WARNING]}),'only known bootstrap warning')
 require(report['cloud_init_exit'] in [0,2] and type(report['cloud_init_exit']) is int,'cloud exit classification')
 for stage in ['init-local','init','modules-config','modules-final']:require(cloud[stage]['errors']==[],'cloud stage error')
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
 require(report.get('guest_host_public_key','').startswith('ssh-ed25519 '),'explicit guest public host identity')
 old=read(directory/'before-reboot-id.stdout').strip();new=read(directory/'reboot-readiness.stdout').strip()
 require(old!=new and all(re.fullmatch(rb'[0-9a-f-]{36}',value) for value in [old,new]),'actual fresh kernel boot')
 before=read(directory/'baseline-inventory.stdout').decode();equal(report['observation'],before,'raw observation binding')
 return semantic(before,read(directory/'reboot-verification.stdout').decode())
def verify(here=HERE):
 provenance=loads(read(here/'provenance.json'))
 require(type(provenance) is dict and set(provenance)=={'files'},'provenance shape')
 actual={str(p.relative_to(here/'evidence')) for p in (here/'evidence').rglob('*') if p.is_file()}
 require(actual==set(provenance['files']),'exact evidence inventory')
 for name,value in provenance['files'].items():
  require(not Path(name).is_absolute() and '..' not in Path(name).parts,'safe evidence path')
  require(digest(here/'evidence'/name)==value,'raw evidence digest '+name)
  require(b'BEGIN OPENSSH PRIVATE KEY' not in read(here/'evidence'/name),'no private key evidence')
 first=verify_guest(here/'evidence/first',here);second=verify_guest(here/'evidence/repeat',here)
 equal(first,second,'independent guest repeated semantics')
 prepare=loads(read(here/'evidence/prepare/receipt.json'))
 require(prepare['helper_sha256']==digest(ROOT/'tools/defaults/capture.py'),'preparation lifecycle helper')
 for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:require(prepare.get(key)==[],'package preparation cleanup')
 require(set(prepare['source_sha256'])=={'Prepare.Dockerfile','go.mod','baseline_test.go','prepare.py'},'preparation source inventory')
 for name,value in prepare['source_sha256'].items():require(digest(here/name)==value,'preparation source')
 inputs=loads(read(here/'inputs.json'))
 equal(prepare['packages'],inputs['packages']['selected'],'full resolved package closure')
 require(prepare['index_sha256']==inputs['packages']['index_sha256'] and prepare['oracle_sha256']==inputs['oracle']['binary_sha256'],'package/oracle input identities')
 require(prepare['baseline_source_and_binary'].splitlines()[0]==inputs['oracle']['source_sha256']+'  preflight.go','unchanged baseline source')
 return first
if __name__=='__main__':verify();print('Alpine guest preparation evidence verified')
