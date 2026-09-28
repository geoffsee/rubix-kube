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

CONSUMER = ['/layer-tests', '--ignored', '--exact', 'verify_pinned_crane_layers',
            '--show-output', '--test-threads=1']

def parse_consumer(output):
    rows = []
    for line in output.splitlines():
        if line.startswith('RUBIX_LAYER '):
            rows.append(oracle.strict(line.removeprefix('RUBIX_LAYER ')))
    for row in rows:
        oracle.observation_schema(row)
    oracle.require([row.get('case') for row in rows] == list(oracle.NAMES), 'exact consumer case order')
    test_lines = [line.strip() for line in output.splitlines()
                  if line.lstrip().startswith('test ') and not line.lstrip().startswith('test result:')]
    oracle.require(test_lines == ['test verify_pinned_crane_layers ... ok'], 'exact consumer test inventory')
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
    with tempfile.TemporaryDirectory(prefix='layer-', dir='/tmp') as temporary:
        directory = Path(temporary)
        subprocess.run(['/producer', temporary], check=True, timeout=20)
        upstream = oracle.strict(read_fixture(directory/'upstream.json'))
        positives = {name:read_fixture(directory/(name+'.tar.gz')) for name in oracle.POSITIVES}
        inputs = oracle.cases(positives, upstream)
        expected = oracle.observations(inputs, upstream)
        print('RUBIX_UPSTREAM '+json.dumps(upstream, separators=(',',':')), flush=True)
        for name, raw in inputs.items():
            (directory/(name+'.tar.gz')).write_bytes(raw)
            print('RUBIX_INPUT '+json.dumps({'case':name, 'gzip_base64':base64.b64encode(raw).decode()}, separators=(',',':')), flush=True)
        oracle.require(bounded is not None, 'owned runtime helper required')
        os.environ['RUBIX_LAYER_FIXTURE'] = temporary
        output_path = directory/'consumer.log'
        try:
            bounded.bounded(CONSUMER, output_path, 20, 65536)
        except Exception:
            # Preserve the already-bounded diagnostic before TemporaryDirectory cleanup.
            # A failed/partial log is evidence of failure, never parsed as a passing run.
            try:
                with output_path.open('rb') as stream:
                    diagnostic = stream.read(65537)
                if len(diagnostic) <= 65536:
                    print(diagnostic.decode('utf-8', errors='replace'), end='', flush=True)
                else:
                    print('Consumer diagnostic exceeded its bound; raw output omitted.', flush=True)
            except OSError:
                # Missing/unreadable diagnostics must not replace the original command error.
                pass
            raise
        output = output_path.read_text()
        observed = parse_consumer(output)
        oracle.require(oracle.same_json(observed, expected), 'independent oracle/consumer equality')
        print(output, end='', flush=True)
        print('RUBIX_COMPLETE '+json.dumps({'cases':list(oracle.NAMES), 'consumer_command':CONSUMER}, separators=(',',':')), flush=True)

if __name__ == '__main__':
    main()
