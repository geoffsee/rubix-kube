"""Run the pinned synthetic producer and the real Rust consumer inside owned Docker only."""
import base64
import json
import os
from pathlib import Path
import subprocess
import tempfile
import oracle
# The runtime image copies the same source-bound helper used by the host driver.
try:
    import bounded
except ModuleNotFoundError:
    bounded = None  # Host verifier imports parsing only, never runtime main().

CONSUMER = ['/archive-tests', '--ignored', '--exact', 'inspect_pinned_crane_serialization',
            '--show-output', '--test-threads=1']

def parse_consumer(output):
    rows = []
    for line in output.splitlines():
        if line.startswith('RUBIX_ARCHIVE '):
            rows.append(oracle.strict(line.removeprefix('RUBIX_ARCHIVE ')))
    oracle.require([row.get('case') for row in rows] == list(oracle.NAMES), 'exact consumer case order')
    oracle.require('test inspect_pinned_crane_serialization ... ok' in output.splitlines(), 'consumer test success')
    summaries = [line for line in output.splitlines() if line.startswith('test result:')]
    import re
    oracle.require(len(summaries) == 1 and re.fullmatch(
        r'test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s', summaries[0]), 'consumer completion')
    return {row['case']:row for row in rows}

def read_fixture(path):
    with path.open('rb') as stream:
        raw = stream.read(oracle.LIMIT + 1)
    oracle.require(len(raw) <= oracle.LIMIT, 'encoded fixture read bound')
    return raw

def main():
    with tempfile.TemporaryDirectory(prefix='archive-', dir='/tmp') as temporary:
        directory = Path(temporary)
        subprocess.run(['/producer', temporary], check=True, timeout=20)
        inputs = oracle.cases(read_fixture(directory/'amd64-repeated.tar.gz'),
                              read_fixture(directory/'armv7-unresolved.tar.gz'))
        expected = oracle.observations(inputs)
        for name, raw in inputs.items():
            (directory/(name+'.tar.gz')).write_bytes(raw)
            print('RUBIX_INPUT '+json.dumps({'case':name, 'gzip_base64':base64.b64encode(raw).decode()}, separators=(',',':')), flush=True)
        oracle.require(bounded is not None, 'owned runtime helper required')
        os.environ['RUBIX_ARCHIVE_FIXTURE'] = temporary
        output_path = directory/'consumer.log'
        bounded.bounded(CONSUMER, output_path, 20, 65536)
        output = output_path.read_text()
        observed = parse_consumer(output)
        oracle.require(observed == expected, 'independent oracle/consumer equality')
        print(output, end='', flush=True)
        print('RUBIX_COMPLETE '+json.dumps({'cases':list(oracle.NAMES), 'consumer_command':CONSUMER}, separators=(',',':')), flush=True)

if __name__ == '__main__':
    main()
