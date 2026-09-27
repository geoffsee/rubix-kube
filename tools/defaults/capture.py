#!/usr/bin/env python3
"""Run official defaulting functions in an owned, network-isolated Linux fixture."""
import argparse
import hashlib
import json
import os
import signal
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import uuid

HERE = Path(__file__).resolve().parent

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bounded(command, path, seconds, limit=8*1024*1024):
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,start_new_session=True)
    def stop():
        try:os.killpg(process.pid,signal.SIGKILL)
        except ProcessLookupError:pass
    errors = []
    def drain():
        total = 0
        try:
            with path.open('xb') as output:
                while block := process.stdout.read1(65536):
                    remaining = max(0, limit-total)
                    output.write(block[:remaining]);output.flush();total += len(block)
                    if total > limit:
                        errors.append('output limit exceeded');stop();break
        except Exception as error:
            errors.append(str(error));stop()
    thread=threading.Thread(target=drain,daemon=True);thread.start()
    try:
        code=process.wait(timeout=seconds)
    except subprocess.TimeoutExpired:
        stop();process.wait(timeout=10);raise
    finally:
        thread.join(timeout=10)
        if thread.is_alive():
            errors.append('output drain did not stop');stop()
        else:
            process.stdout.close()
    if errors or code:
        raise RuntimeError(f'command failed ({code}): {errors}; see {path}')


def finish(report,output,tag):
    def cleanup(label,command):
        try:
            result=subprocess.run(command,check=True,capture_output=True,timeout=30)
            return result.stdout.decode().splitlines()
        except Exception as error:
            report['cleanup_errors'].append(label+': '+str(error));return None
    for name in report['containers']:cleanup('remove '+name,['docker','rm','--force',name])
    cleanup('remove image',['docker','image','rm','--force',tag])
    report['remaining_containers']=cleanup('inspect containers',['docker','ps','-aq','--filter','name='+tag])
    report['remaining_images']=cleanup('inspect images',['docker','images','-q',tag])
    (output/'receipt.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-archive',type=Path,required=True)
    parser.add_argument('--go-archive',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    inputs=json.loads((HERE/'inputs.json').read_text())
    for kind,path in [('source',args.source_archive),('go',args.go_archive)]:
        if path.stat().st_size!=inputs[kind]['bytes'] or digest(path)!=inputs[kind]['sha256']:
            raise ValueError(f'{kind} input hash mismatch')
    args.output.mkdir(parents=True,exist_ok=False)
    tag='rubix-official-defaults-'+uuid.uuid4().hex
    report={'inputs':inputs,'source_sha256':{name:digest(HERE/name) for name in ('capture.py','main.go','apiserver.go','Dockerfile','inputs.json')},'image':tag,'containers':[],'errors':[],'cleanup_errors':[]}
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-official-build-') as temporary:
            work=Path(temporary)
            shutil.copyfile(args.source_archive,work/'source.tar.gz');shutil.copyfile(args.go_archive,work/'go.tar.gz')
            for name in ('main.go','apiserver.go','Dockerfile'):shutil.copyfile(HERE/name,work/name)
            bounded(['docker','build','--network','none','--platform','linux/arm64','--tag',tag,'--build-arg','SOURCE_SHA256='+inputs['source']['sha256'],str(work)],args.output/'build.log',1800)
        report['image_inspect']=json.loads(subprocess.check_output(['docker','image','inspect',tag],timeout=30))
        for variant,binary in [('run','extract'),('apiserver','extract-apiserver')]:
            for index in range(2):
                name=tag+'-'+variant+'-'+str(index);report['containers'].append(name)
                bounded(['docker','run','--name',name,'--hostname','fixture-'+str(index),'--entrypoint','/out/'+binary,'--network','none','--read-only','--cap-drop','ALL','--security-opt','no-new-privileges','--memory','512m','--cpus','2','--pids-limit','64','--tmpfs','/tmp:rw,nosuid,nodev,size=16m',tag],args.output/f'{variant}{index}.json',60,2*1024*1024)
            if (args.output/f'{variant}0.json').read_bytes()!=(args.output/f'{variant}1.json').read_bytes():
                raise ValueError('nondeterministic extraction: '+variant)
        report['identical_repeats']=True
        report['hostnames']=['fixture-0','fixture-1']
        report['output_sha256']=digest(args.output/'run0.json')
        report['apiserver_output_sha256']=digest(args.output/'apiserver0.json')
        base=json.loads((args.output/'run0.json').read_text())['registered_feature_gates']
        api=json.loads((args.output/'apiserver0.json').read_text())['registered_feature_gates']
        report['registry_difference']={'added':sorted(api.keys()-base.keys()),'removed':sorted(base.keys()-api.keys()),'changed':sorted(key for key in base.keys()&api.keys() if base[key]!=api[key])}
        # Only trusted build metadata from a scratch image, never private fixture state.
        subprocess.run(['docker','cp',report['containers'][0]+':/out/modules.sha256',str(args.output/'modules.sha256')],check=True,timeout=30)
    except Exception as error:
        report['errors'].append(str(error))
    finally:
        finish(report,args.output,tag)
    return 1 if report['errors'] or report['cleanup_errors'] or report['remaining_containers'] or report['remaining_images'] else 0

if __name__=='__main__':
    raise SystemExit(main())
