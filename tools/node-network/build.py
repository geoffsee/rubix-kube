#!/usr/bin/env python3
"""Build static Linux network consumer; execute injected tests and effect-free CLI paths only."""
import argparse
import json
from pathlib import Path
import shutil
import tempfile
import uuid
from support import HERE, ROOT, digest, inventory, module, require
import verify_build
helper=module('network_build_helper',ROOT/'tools/defaults/capture.py')

def control(argv):
    with tempfile.TemporaryDirectory() as temporary:
        path=Path(temporary)/'log';helper.bounded(argv,path,30,65536)
        return path.read_text().strip()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--allow-dirty-hostsafe-build',action='store_true')
    args=parser.parse_args()
    source=inventory()
    revision=control(['git','-C',str(ROOT),'rev-parse','HEAD'])
    dirty=bool(control(['git','-C',str(ROOT),'status','--porcelain','--untracked-files=all','--',*sorted(source)]))
    require(not dirty or args.allow_dirty_hostsafe_build,'source must be committed before qualification')
    args.output.mkdir(parents=True,exist_ok=False)
    tag='rubix-node-network-'+uuid.uuid4().hex
    report=dict(schema=1,revision=revision,dirty=dirty,tag=tag,containers=[],errors=[],cleanup_errors=[],helper_sha256=digest(ROOT/'tools/defaults/capture.py'))
    try:
        with tempfile.TemporaryDirectory(prefix='rubix-network-build-') as temporary:
            context=Path(temporary)
            for name in source:
                destination=context/name;destination.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(ROOT/name,destination)
            (args.output/'source-hashes.json').write_text(json.dumps(source,sort_keys=True,indent=2)+'\n')
            report['source_sha256']=digest(args.output/'source-hashes.json')
            helper.bounded(['docker','build','--platform=linux/arm64','-t',tag,'-f',str(context/'tools/node-network/Build.Dockerfile'),str(context)],args.output/'build.log',1800,32*1024*1024)
        report['build_sha256']=digest(args.output/'build.log',32*1024*1024)
        report['image_id']=control(['docker','image','inspect','--format','{{.Id}}',tag])
        report['containers']=[tag+'-test'];report['command']=verify_build.command(tag)
        helper.bounded(report['command'],args.output/'run.log',120,1024*1024)
        report['run_sha256']=digest(args.output/'run.log')
        helper.bounded(['docker','cp',tag+'-test:/out/prepare_host_network',str(args.output/'prepare_host_network')],args.output/'copy.log',30,65536)
        artifact=args.output/'prepare_host_network'
        metadata=dict(sha256=digest(artifact,64*1024*1024),size=artifact.stat().st_size,target=verify_build.TARGET,revision=revision)
        (args.output/'artifact.json').write_text(json.dumps(metadata,sort_keys=True,indent=2)+'\n')
        require(inventory()==source,'source changed during build')
    except Exception as error:
        report['errors'].append(str(error))
    finally:
        helper.finish(report,args.output,tag)
    verify_build.verify(args.output,frozen=not args.allow_dirty_hostsafe_build)
    return 0
if __name__=='__main__':raise SystemExit(main())
