"""Read-only static Linux consumer build and host-safe test receipt verification."""
from pathlib import Path
import re
import hashlib
import sys
from support import HERE, ROOT, digest, load, loads, read, require, inventory
TARGET = 'aarch64-unknown-linux-musl'

TESTS = {
    'complete_bounded_scalars_reject_prefixes_and_binary_data',
    'owner_launch_failure_has_no_child_and_is_settled',
    'cancellation_before_start_never_observes_or_mutates_network',
    'unjoined_module_owner_stops_all_further_effects',
    'fresh_root_backend_and_ipv4_unknown_guards_prevent_effects',
    'cancellation_after_ipv6_write_preserves_possible_effects_without_readback',
    'cancellation_between_ipv6_read_and_write_prevents_write',
    'cancellation_after_failed_write_retains_possible_effects',
    'ipv6_only_zero_writes_and_success_requires_readback',
    'write_failures_continue_to_all_controls_and_retain_possible_effects',
    'requested_unknown_ipv6_is_deferred_only_by_preparation',
    'fixed_backend_lists_and_external_runtime_keep_network_effects',
    'settled_module_failures_warn_and_continue_without_loaded_claim',
    'cancellation_during_module_waits_for_stop_acknowledgement',
}
GUEST_FLAGS = '--disable-ipv6 --no-container-mode --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock'

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
    for section,expected in [('network','  disableIPv6: true'),
                             ('runtime','  containerMode: false'),
                             ('runtime','  endpoint: unix:///tmp/external-runtime/containerd.sock')]:
        require(lines.count(expected)==1 and sections.get(section,[]).count(expected)==1,
                'effective guest configuration '+section+'.'+expected.strip())
    require('"shared_effects_possible"' not in text,'configuration exits before preparation')

def verify_run(raw):
    text=raw.decode();lines=text.splitlines()
    require(sum(line.startswith('test result: ok. 14 passed; 0 failed;') for line in lines)==1,'14 safe tests')
    tests=[line.removeprefix('test ').removesuffix(' ... ok') for line in lines if line.startswith('test ') and not line.startswith('test result:')]
    require(len(tests)==14 and set(tests)==TESTS,'exact safe test inventory')
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
    return ['docker', 'run', '--name', tag+'-test', '--network=none', '--read-only',
            '--cap-drop=ALL', '--security-opt=no-new-privileges', '--pids-limit=64',
            '--memory=512m', '--cpus=2', tag, '/bin/sh', '-c',
            'sha256sum /out/prepare_host_network /out/host_network && /out/host_network && for case in version help print-config; do echo CLI_${case}_BEGIN; if [ "$case" = print-config ]; then /out/prepare_host_network '+GUEST_FLAGS+' --print-config 2>&1 || exit 1; else /out/prepare_host_network --$case 2>&1 || exit 1; fi; echo CLI_${case}_END; done']

def verify(directory, binary=True, frozen=True):
    report = load(directory/'receipt.json')
    require(type(report.get('schema')) is int and report['schema']==1, 'build schema')
    require(re.fullmatch('[a-f0-9]{40}', report.get('revision','')) is not None, 'build revision')
    require(type(report.get('dirty')) is bool and (not frozen or not report['dirty']), 'frozen build')
    tag = report.get('tag','')
    require(re.fullmatch('rubix-node-network-[a-f0-9]{32}', tag) is not None, 'owned build tag')
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
    lines=verify_run(read(directory/'run.log'))
    hashes = {}
    for line in lines:
        match=re.fullmatch('([a-f0-9]{64})  /out/(prepare_host_network|host_network)',line)
        if match:
            value,name=match.groups();require(name not in hashes,'duplicate binary');hashes[name]=value
    require(set(hashes)=={'prepare_host_network','host_network'},'binary inventory')
    metadata=load(directory/'artifact.json')
    require(set(metadata)=={'sha256','size','target','revision'},'artifact shape')
    require(metadata['sha256']==hashes['prepare_host_network'] and metadata['target']==TARGET and metadata['revision']==report['revision'],'artifact binding')
    require(type(metadata['size']) is int and 0<metadata['size']<64*1024*1024,'artifact size')
    if binary:
        raw=read(directory/'prepare_host_network',64*1024*1024)
        require(len(raw)==metadata['size'] and hashlib.sha256(raw).hexdigest()==metadata['sha256'],'artifact bytes')
    return metadata
if __name__=='__main__':
    verify(Path(sys.argv[1]));print('network build verified')
