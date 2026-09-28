"""Verify trusted repository evidence; never execute an artifact during verification."""
import hashlib
import json
import math
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FILES = {'evidence/build.log', 'evidence/run.log', 'evidence/receipt.json', 'evidence/source-hashes.json'}
SUMMARY = 'RUBIX_QUALIFICATION repetitions=20 full_partial_signal_cases=160 worker_failure_cases=20 all_children_reaped=true'
BUILDER = 'rust@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97'
RECEIPT_KEYS = {'builder', 'cleanup_errors', 'containers', 'errors', 'helper_sha256', 'image_id', 'owned_subprocesses_reaped', 'platform', 'remaining_containers', 'remaining_images', 'repetitions', 'run_sha256', 'schema_version', 'scope', 'signal_cases', 'source_revision', 'source_sha256', 'test_binary_sha256', 'uncommitted_implementation', 'worker_failure_cases', 'owned_binary_sha256', 'combined_cases'}

def require(condition, message):
    if not condition:
        raise ValueError(message)

def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result

def finite(value):
    result = float(value)
    require(math.isfinite(result), 'nonfinite JSON number')
    return result

def loads(value):
    def invalid(_):
        raise ValueError('nonfinite JSON constant')
    return json.loads(value, object_pairs_hook=pairs, parse_float=finite, parse_constant=invalid)

def read(path):
    require(path.is_file() and not path.is_symlink(), 'expected regular evidence file')
    require(path.stat().st_size <= 16 * 1024 * 1024, 'evidence size limit')
    return path.read_bytes()

def digest(path):
    return hashlib.sha256(read(path)).hexdigest()

def sha(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None

def validate_receipt(receipt, log):
    require(type(receipt) is dict and set(receipt) == RECEIPT_KEYS, 'receipt keys')
    for key, value in [('schema_version', 1), ('repetitions', 20), ('signal_cases', 160), ('worker_failure_cases', 20), ('combined_cases', 61)]:
        require(type(receipt[key]) is int and receipt[key] == value, 'receipt count: ' + key)
    for key in ['owned_subprocesses_reaped', 'uncommitted_implementation']:
        require(receipt[key] is True, 'receipt boolean: ' + key)
    for key in ['errors', 'cleanup_errors', 'remaining_containers', 'remaining_images']:
        require(type(receipt[key]) is list and not receipt[key], 'unclean receipt: ' + key)
    require(receipt['builder'] == BUILDER and receipt['platform'] == 'linux/arm64', 'platform or toolchain')
    require(isinstance(receipt['source_revision'], str) and re.fullmatch('[0-9a-f]{40}', receipt['source_revision']), 'historical revision')
    require(receipt['scope'] == 'cooperative task signals and owned process adapter signals; no cluster or escaped-daemon qualification', 'scope')
    require(isinstance(receipt['image_id'], str) and re.fullmatch('sha256:[0-9a-f]{64}', receipt['image_id']), 'image id')
    require(type(receipt['containers']) is list and len(receipt['containers']) == 1 and re.fullmatch('rubix-signal-capture-[0-9a-f]{32}-test', receipt['containers'][0]), 'owned container identity')
    for key in ['test_binary_sha256', 'owned_binary_sha256', 'run_sha256', 'helper_sha256']:
        require(sha(receipt[key]), 'hash: ' + key)
    require(type(receipt['source_sha256']) is dict and set(receipt['source_sha256']) == {'capture.py', 'verify.py', 'Capture.Dockerfile', 'source-hashes.json'}, 'source inventory keys')
    require(all(sha(value) for value in receipt['source_sha256'].values()), 'source hashes')
    require(hashlib.sha256(log).hexdigest() == receipt['run_sha256'], 'raw log hash')
    lines = log.decode().splitlines()
    validate_owned(lines)
    require(lines.count(receipt['owned_binary_sha256'] + '  /out/owned_signals.test') == 1, 'owned binary hash')
    require(lines.count(SUMMARY) == 1, 'completed repetition record')
    require(lines.count(receipt['test_binary_sha256'] + '  /out/signals.test') == 1, 'executed binary hash')
    require(len([line for line in lines if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s', line)]) == 1, 'test completion')

def validate_owned(lines):
    summary='RUBIX_OWNED_QUALIFICATION repetitions=20 combined_cases=61 sentinel_survived=true all_owners_joined=true'
    require(lines.count(summary)==1, 'owned completion summary')
    rows=[line for line in lines if line.startswith('RUBIX_OWNED_SIGNAL ')]
    expected=[(mode,i) for i in range(20) for mode in ['full','partial','fatal']]+[('force',20)]
    require(len(rows)==len(expected), 'owned case inventory')
    for line,(mode,iteration) in zip(rows,expected):
        prefix=f'RUBIX_OWNED_SIGNAL case={mode} iteration={iteration} leader_reaped=true owner_joined=true dependent_started={str(mode in ["full","force"]).lower()} elapsed_ms='
        require(line.startswith(prefix) and line[len(prefix):].isdigit(), 'owned cleanup case')
        elapsed=int(line[len(prefix):])
        require(0 <= elapsed < 38000 and (mode!='force' or elapsed>=29000), 'owned timing')
    namespace=[line for line in lines if line.startswith('RUBIX_NAMESPACE ')]
    require(len(namespace)==1, 'namespace count')
    record=loads(namespace[0].removeprefix('RUBIX_NAMESPACE '))
    require(type(record) is dict and set(record)=={'init','shell','helper','processes'}, 'namespace keys')
    require(all(type(record[key]) is int for key in ['init','shell','helper']) and record['init']==1 and record['shell']>1 and record['helper']>1 and record['shell']!=record['helper'], 'namespace identities')
    require(type(record['processes']) is list and all(type(pid) is int for pid in record['processes']) and len(record['processes'])==3 and set(record['processes'])=={record['init'],record['shell'],record['helper']}, 'namespace residue')
    require(len([line for line in lines if re.fullmatch(r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s',line)])==1, 'owned test completion')

def verify(here=HERE, root=ROOT):
    provenance = loads(read(here / 'provenance.json'))
    require(type(provenance) is dict and set(provenance) == {'schema_version', 'files'} and type(provenance['schema_version']) is int and provenance['schema_version'] == 1, 'provenance schema')
    require(type(provenance['files']) is dict and set(provenance['files']) == FILES, 'exact evidence inventory')
    require({str(path.relative_to(here)) for path in (here / 'evidence').iterdir()} == FILES, 'unexpected evidence entry')
    for name, expected in provenance['files'].items():
        require(sha(expected) and digest(here / name) == expected, 'evidence hash: ' + name)
    receipt = loads(read(here / 'evidence/receipt.json'))
    validate_receipt(receipt, read(here / 'evidence/run.log'))
    for name, expected in receipt['source_sha256'].items():
        path = here / ('evidence/' + name if name == 'source-hashes.json' else name)
        require(digest(path) == expected, 'capture source hash: ' + name)
    require(digest(root / 'tools/defaults/capture.py') == receipt['helper_sha256'], 'capture lifecycle helper')
    inventory = loads(read(here / 'evidence/source-hashes.json'))
    require(type(inventory) is dict and all(sha(value) for value in inventory.values()), 'historical source inventory')
    required = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'crates/rubix-supervisor/Cargo.toml', 'crates/rubix-supervisor/src/lib.rs', 'crates/rubix-supervisor/src/signals.rs', 'crates/rubix-supervisor/tests/signals.rs', 'Dockerfile'}
    require(required <= set(inventory), 'missing compiled input')
    # All Cargo manifests influence workspace resolution, even unrelated crate manifests.
    bound = {name for name in inventory if name.endswith('Cargo.toml') or name in {'Cargo.lock', 'rust-toolchain.toml'} or name.startswith('.cargo/') or (name.startswith('crates/rubix-supervisor/') and name.endswith('.rs'))}
    current = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'}
    current.update(str(path.relative_to(root)) for path in (root / '.cargo').rglob('*') if path.is_file())
    for directory in ['crates', 'third_party', 'tools/upstream']:
        current.update(str(path.relative_to(root)) for path in (root / directory).rglob('Cargo.toml') if 'target' not in path.parts)
    current.update(str(path.relative_to(root)) for path in (root / 'crates/rubix-supervisor').rglob('*.rs') if 'target' not in path.parts)
    require(current == bound, 'compiled input inventory changed; recapture required')
    for name in bound:
        require(digest(root / name) == inventory[name], 'compiled input changed; recapture required: ' + name)
    for name in ['tools/supervisor-process/fixture.py','tools/supervisor-process/namespace_inventory.py','tools/supervisor-signals/kill.sh']:
        require(name in inventory and digest(root/name)==inventory[name], 'runtime fixture drift')
    require(inventory['Dockerfile'] == digest(here / 'Capture.Dockerfile'), 'build recipe')
    return receipt

if __name__ == '__main__':
    verify()
    print('Signal qualification evidence verified')
