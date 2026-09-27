#!/usr/bin/env python3
"""Run startup-only cases using the pinned baseline binary and existing parity runner."""
import argparse,hashlib,json,pathlib,subprocess,sys
import verify
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[3]
BINARY_SHA256='67348b2560d0f831de20ef722c53cc317e385ab7cfbf406633fbbb6a7b73f81e'
REVISION='2ef1c4787989f11f868f81bb84ae2afd4a49a81d'
def check_runner(path):
 runner=path.resolve()
 pins=verify.load(HERE/'provenance.json')['runner_source_sha256']
 verify.equal(set(pins),verify.RUNNER_KEYS)
 verify.equal(verify.digest(runner),pins['run.py'])
 verify.equal(verify.digest(runner.parent/'driver.py'),pins['driver.py'])
 return runner

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--artifact',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--runner',type=pathlib.Path,default=ROOT/'tools/parity/run.py');a=p.parse_args()
 runner=check_runner(a.runner)
 descriptor=json.loads(a.artifact.read_text());binary=a.artifact.parent/descriptor['binary']
 if descriptor['sha256']!=BINARY_SHA256 or descriptor['source']['revision']!=REVISION or hashlib.sha256(binary.read_bytes()).hexdigest()!=BINARY_SHA256:raise ValueError('unexpected baseline artifact identity')
 return subprocess.run([sys.executable,str(runner),'--artifact',str(a.artifact),'--suite',str(HERE/'suite.json'),'--output',str(a.output)],timeout=900,check=False).returncode
if __name__=='__main__':raise SystemExit(main())
