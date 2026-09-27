#!/usr/bin/env python3
"""Build the pinned upstream external_deps variant without running the distribution."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
source = Path(__file__).resolve().parent
image = f"rubix-parity-build:{uuid.uuid4().hex}"
container = f"rubix-parity-build-{uuid.uuid4().hex}"
errors = []
status = 1


def command(argv, timeout=60, log=None):
    with (args.output / (log or "docker.log")).open("a") as stream:
        return subprocess.run(argv, stdout=stream, stderr=subprocess.STDOUT, timeout=timeout, check=True)


try:
    command(["docker", "build", "--file", str(source / "Upstream.Dockerfile"),
             "--tag", image, str(source)], timeout=1800, log="build.log")
    command(["docker", "create", "--name", container, image, "/not-executed"])
    command(["docker", "cp", f"{container}:/artifact/.", str(args.output)])
    details = subprocess.check_output(["docker", "image", "inspect", image], timeout=30)
    (args.output / "image-inspect.json").write_bytes(details)
    artifact = {
        "schema_version": 1, "kind": "go", "binary": "kubesolo",
        "sha256": hashlib.sha256((args.output / "kubesolo").read_bytes()).hexdigest(),
        "version": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d",
        "source": {"repository": "https://github.com/portainer/kubesolo",
                   "revision": "2ef1c4787989f11f868f81bb84ae2afd4a49a81d"},
        "build": {"variant": "external_deps", "toolchain": "go1.26.5", "cgo": True,
                  "dockerfile_sha256": hashlib.sha256((source / "Upstream.Dockerfile").read_bytes()).hexdigest(),
                  "source_archive_sha256": "9d5f3ce1f3fbda971fb1e2fb6da18ae3880caeec677f0e5d928a3bbe7bb76aec"},
    }
    (args.output / "artifact.json").write_text(json.dumps(artifact, indent=2) + "\n")
    status = 0
except (OSError, subprocess.SubprocessError) as error:
    errors.append(str(error))
finally:
    for object_type, target, remove in [("container", container, ["rm", "--force"]),
                                        ("image", image, ["image", "rm"])]:
        try:
            listed = subprocess.check_output(
                ["docker", object_type, "ls", "--all", "--filter",
                 (f"name=^/{target}$" if object_type == "container" else f"reference={target}"),
                 "--quiet"], timeout=30).strip()
            if listed:
                command(["docker", *remove, target])
            remaining = subprocess.check_output(
                ["docker", object_type, "ls", "--all", "--filter",
                 (f"name=^/{target}$" if object_type == "container" else f"reference={target}"),
                 "--quiet"], timeout=30).strip()
            if remaining:
                errors.append(f"owned {object_type} remains: {target}")
        except (OSError, subprocess.SubprocessError) as error:
            errors.append(str(error))
    if errors:
        status = 1
    (args.output / "preparation-result.json").write_text(json.dumps({
        "exit_code": status, "errors": errors, "owned_container": container, "owned_image": image,
    }, indent=2) + "\n")
raise SystemExit(status)
