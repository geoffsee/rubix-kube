"""Read-only static Linux consumer build and host-safe test receipt verification."""
from pathlib import Path
import re
import hashlib
import sys
from support import HERE, ROOT, digest, load, loads, read, require, inventory
TARGET = 'aarch64-unknown-linux-musl'

BINARIES = ['prepare_node_host', 'rubix_kube', 'host_preparation', 'host_network']
GUEST_FLAGS = '--container-mode --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock'

def verify_config(text):
    # Validate the deterministic emitter's top-level sections without inventing a
    # general YAML parser. apiVersion need not be the first key.
    lines=text.splitlines()
    for expected in ['apiVersion: kubesolo.io/v1alpha1', 'kind: Config']:
        require(lines.count(expected)==1,'root configuration field '+expected)
    sections={}
    current=None
    for line in lines:
        if line and not line[0].isspace():
            match=re.fullmatch(r'([A-Za-z][A-Za-z0-9]*):',line)
            current=match.group(1) if match else None
            if current is not None:
                require(current not in sections,'duplicate configuration section')
                sections[current]=[]
        elif current is not None:
            sections[current].append(line)
    for section,expected in [('network','  disableIPv6: false'),
                             ('runtime','  containerMode: true'),
                             ('runtime','  endpoint: unix:///tmp/external-runtime/containerd.sock')]:
        require(lines.count(expected)==1 and sections.get(section,[]).count(expected)==1,
                'effective guest configuration '+section+'.'+expected.strip())
    require('"shared_effects_possible"' not in text,'configuration exits before preparation')

def verify_tests(text):
    expected=load(HERE/'test-inventory.json')
    for binary,names in expected.items():
        begin='TEST_'+binary+'_BEGIN\n';end='TEST_'+binary+'_END\n'
        require(text.count(begin)==1 and text.count(end)==1,'test frames')
        body=text.split(begin)[1].split(end)[0]
        actual=[line for line in body.splitlines() if line.startswith('test ') and not line.startswith('test result:')]
        require(len(actual)==len(names) and set(actual)=={'test '+name+' ... ok' for name in names},'exact '+binary+' tests')
        require(body.count(f'test result: ok. {len(names)} passed; 0 failed; 0 ignored;')==1,'test summary')

def verify_run(raw):
    text=raw.decode();lines=text.splitlines()
    verify_tests(text)
    for name in ['version','help','print-config']:
        begin='CLI_'+name+'_BEGIN\n';end='CLI_'+name+'_END\n'
        require(text.count(begin)==1 and text.count(end)==1,'CLI complete frames')
        body=text.split(begin)[1].split(end)[0]
        if name=='version':
            require(loads(body)=={'level':'info','message':'kubesolo version','version':'0.1.0'},'version output')
        elif name=='help':
            require(body==read(ROOT/'crates/rubix-kube/src/help.txt').decode(),'complete help output')
        else:
            verify_config(body)
    return lines

def command(tag):
    script='sha256sum '+' '.join('/out/'+name for name in BINARIES)+' && '
    script+='for binary in rubix_kube host_preparation host_network; do echo TEST_${binary}_BEGIN; /out/$binary || exit 1; echo TEST_${binary}_END; done && '
    script+='for case in version help print-config; do echo CLI_${case}_BEGIN; if [ "$case" = print-config ]; then /out/prepare_node_host '+GUEST_FLAGS+' --print-config 2>&1 || exit 1; else /out/prepare_node_host --$case 2>&1 || exit 1; fi; echo CLI_${case}_END; done'
    return ['docker','run','--name',tag+'-test','--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=64','--memory=512m','--cpus=2',tag,'/bin/sh','-c',script]

def build_command(tag,nonce,context):
    return ['docker','build','--platform=linux/arm64','--progress=plain','--build-arg','QUALIFICATION_NONCE='+nonce,'-t',tag,'-f',context+'/tools/node-container/Build.Dockerfile',context]

def builder_hashes(raw,nonce):
    require(re.fullmatch('[a-f0-9]{32}',nonce) is not None,'builder nonce')
    text='\n'.join(re.sub(r'^#[0-9]+ [0-9]+(?:\.[0-9]+)? ', '', line) for line in raw.decode().splitlines())+'\n'
    require(text.count('POLICY_TESTS_BEGIN\n')==1 and text.count('POLICY_TESTS_END\n')==1,'consumer policy test frames')
    policy=text.split('POLICY_TESTS_BEGIN\n')[1].split('POLICY_TESTS_END\n')[0]
    expected={'tests::cancelled_result_publishes_once_and_is_terminal','tests::uncertain_cleanup_is_quiet_terminal_even_when_observer_is_settled'}
    tested=[line for line in policy.splitlines() if line.startswith('test ') and not line.startswith('test result:')]
    require(len(tested)==2 and set(tested)=={'test '+name+' ... ok' for name in expected} and policy.count('test result: ok. 2 passed; 0 failed; 0 ignored;')==1,'consumer quiet/terminal policy tests')
    begin='RUBIX_BUILD_BIND_BEGIN '+nonce+'\n';end='RUBIX_BUILD_BIND_END '+nonce+'\n'
    require(text.count(begin)==1 and text.count(end)==1,'independent builder hash frame')
    body=text.split(begin)[1].split(end)[0]
    rows=re.findall(r'^([a-f0-9]{64})  /out/([a-z_]+)$',body,re.M)
    require(len(rows)==len(BINARIES) and {name for _,name in rows}==set(BINARIES),'independent builder binaries')
    require(body==''.join(digest+'  /out/'+name+'\n' for digest,name in rows),'builder frame exact bytes')
    return {name:digest for digest,name in rows}

def verify(directory, binary=True, frozen=True):
    report = load(directory/'receipt.json')
    require(type(report.get('schema')) is int and report['schema']==1, 'build schema')
    require(re.fullmatch('[a-f0-9]{40}', report.get('revision','')) is not None, 'build revision')
    require(type(report.get('dirty')) is bool and (not frozen or not report['dirty']), 'frozen build')
    tag = report.get('tag','')
    require(re.fullmatch('rubix-node-container-[a-f0-9]{32}', tag) is not None, 'owned build tag')
    require(report.get('containers')==[tag+'-test'], 'owned build containers')
    require(report.get('command')==command(tag), 'isolated host-safe build test')
    for key in ['errors','cleanup_errors','remaining_containers','remaining_images']:
        require(report.get(key)==[], 'build cleanup '+key)
    require(re.fullmatch('sha256:[a-f0-9]{64}', report.get('image_id','')) is not None, 'build image')
    require(report.get('helper_sha256')==digest(ROOT/'tools/defaults/capture.py'), 'build helper')
    require(report.get('source_sha256')==digest(directory/'source-hashes.json'), 'source inventory binding')
    require(load(directory/'source-hashes.json')==inventory(), 'current compiled and harness inputs')
    for label in ['build','run']:
        require(report.get(label+'_sha256')==digest(directory/(label+'.log'),32*1024*1024), 'raw '+label)
    nonce=report.get('build_nonce','')
    command_value=report.get('build_command',[])
    require(type(command_value) is list and command_value and type(command_value[-1]) is str and re.fullmatch(r'/tmp/rubix-container-build-[a-zA-Z0-9_]+',command_value[-1]),'owned build context')
    require(command_value==build_command(tag,nonce,command_value[-1]),'exact builder command')
    built=builder_hashes(read(directory/'build.log',32*1024*1024),nonce)
    lines=verify_run(read(directory/'run.log'))
    hashes={}
    for line in lines:
        match=re.fullmatch('([a-f0-9]{64})  /out/('+'|'.join(BINARIES)+')',line)
        if match:
            value,name=match.groups();require(name not in hashes,'duplicate binary');hashes[name]=value
    require(set(hashes)==set(BINARIES) and hashes==built,'builder/runtime binary inventory binding')
    metadata=load(directory/'artifact.json')
    require(set(metadata)=={'files','target','revision'},'artifact shape')
    require(metadata['target']==TARGET and metadata['revision']==report['revision'],'artifact target/revision')
    require(set(metadata['files'])==set(BINARIES),'artifact files')
    for name,item in metadata['files'].items():
        require(set(item)=={'sha256','size'} and item['sha256']==hashes[name],'artifact binding')
        require(type(item['size']) is int and 0<item['size']<64*1024*1024,'artifact size')
        if binary:
            raw=read(directory/name,64*1024*1024)
            require(len(raw)==item['size'] and hashlib.sha256(raw).hexdigest()==item['sha256'],'artifact bytes')
    return metadata
if __name__=='__main__':
    verify(Path(sys.argv[1]));print('container build verified')
