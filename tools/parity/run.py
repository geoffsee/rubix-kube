#!/usr/bin/env python3
"""Run the same Linux black-box suite against a checksum-pinned Go or Rust artifact."""
import argparse
import os
import resource
import tarfile
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import uuid

BASE = "python:3.13.7-slim-bookworm@sha256:adafcc17694d715c905b4c7bebd96907a1fd5cf183395f0ebc4d3428bd22d92d"


def validate(artifact, suite):
    if artifact.get("schema_version") != 1 or artifact.get("kind") not in ("go", "rust"):
        raise ValueError("artifact schema_version=1 and kind=go|rust are required")
    if not re.fullmatch(r"[a-f0-9]{64}", artifact.get("sha256", "")):
        raise ValueError("artifact requires a SHA-256 digest")
    if not re.fullmatch(r"[a-f0-9]{40}", artifact.get("source", {}).get("revision", "")):
        raise ValueError("artifact source revision must be an exact 40-character Git commit")
    if not artifact["source"].get("repository") or not artifact.get("version"):
        raise ValueError("artifact repository and version are required")
    if suite.get("schema_version") != 1 or not suite.get("id") or not suite.get("cases"):
        raise ValueError("nonempty schema_version=1 suite required")
    if len(suite["cases"]) > 64:
        raise ValueError("a suite may contain at most 64 cases")
    seen = set()
    for case in suite["cases"]:
        if case.get("id") in seen or not isinstance(case.get("id"), str):
            raise ValueError("case IDs must be unique strings")
        seen.add(case["id"])
        if not isinstance(case.get("argv"), list) or not all(isinstance(v, str) for v in case["argv"]):
            raise ValueError("case argv must be an array of strings")
        if not isinstance(case.get("expect", {}).get("exit_code"), int):
            raise ValueError("each case requires an expected integer exit code")
        if not 0 < case.get("timeout_seconds", 30) <= 120:
            raise ValueError("case timeout must be within 1..120 seconds")
        if case.get("privilege", "none") not in ("none", "privileged"):
            raise ValueError("unsupported privilege mode")
        if not all(isinstance(k, str) and isinstance(v, str) for k, v in case.get("env", {}).items()):
            raise ValueError("case environment must map strings to strings")
        for key, value in case["expect"].items():
            if key == "exit_code":
                continue
            if key not in {f"{stream}_{mode}" for stream in ("stdout", "stderr", "combined")
                           for mode in ("contains", "equals")}:
                raise ValueError(f"unknown expectation: {key}")
            if key.endswith("_contains"):
                if not isinstance(value, list) or not all(isinstance(v, str) for v in value):
                    raise ValueError("contains expectations must be string arrays")
            elif not isinstance(value, str):
                raise ValueError("equals expectations must be strings")


def archive_limit():
    resource.setrlimit(resource.RLIMIT_FSIZE, (64 * 1024 * 1024, 64 * 1024 * 1024))


def publish_evidence(archive, destination):
    """Validate the whole untrusted archive, then publish only regular named reports."""
    members = archive.getmembers()
    allowed = re.compile(r"(?:result\.json|[0-9]{3}\.(?:stdout|stderr))")
    selected = []
    names = set()
    for member in members:
        name = member.name.removeprefix("./")
        if member.isdir() and name in (".", ""):
            continue
        if not member.isfile() or not allowed.fullmatch(name) or name in names:
            raise ValueError(f"unsafe or unexpected evidence member: {member.name}")
        maximum = 2 * 1024 * 1024 if name == "result.json" else 256 * 1024
        if member.size > maximum:
            raise ValueError(f"oversized evidence member: {name}")
        names.add(name)
        selected.append((member, name))
    for member, name in selected:
        descriptor = os.open(destination / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, "wb") as output, archive.extractfile(member) as content:
            shutil.copyfileobj(content, output, 64 * 1024)


def copy_evidence(container, destination):
    # Never docker-cp container-controlled entries directly into a host directory.
    with tempfile.TemporaryFile() as quarantine:
        subprocess.run(["docker", "cp", f"{container}:/evidence/.", "-"], stdout=quarantine,
                       stderr=subprocess.PIPE, check=True, timeout=30, preexec_fn=archive_limit)
        if quarantine.tell() >= 64 * 1024 * 1024:
            raise ValueError("evidence archive exceeded 64 MiB")
        quarantine.seek(0)
        with tarfile.open(fileobj=quarantine, mode="r:") as archive:
            publish_evidence(archive, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--suite", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--inject-failure", choices=("setup", "test"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).resolve().parent
    owned = f"rubix-parity-{uuid.uuid4().hex}"
    image = owned + ":test"
    volume = owned + "-evidence"
    errors = []
    status = 1
    hashes = {name: hashlib.sha256((source / name).read_bytes()).hexdigest()
              for name in ("run.py", "driver.py")}

    def command(label, argv, timeout=30, capture=False):
        try:
            completed = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       timeout=timeout, check=True)
            (args.output / f"{label}.stdout").write_bytes(completed.stdout)
            (args.output / f"{label}.stderr").write_bytes(completed.stderr)
            return completed.stdout.decode() if capture else True
        except (OSError, subprocess.SubprocessError) as error:
            for stream in ("stdout", "stderr"):
                data = getattr(error, stream, None)
                if data:
                    (args.output / f"{label}.{stream}").write_bytes(data if isinstance(data, bytes) else data.encode())
            errors.append({"operation": label, "error": str(error)})
            return None

    try:
        artifact = json.loads(args.artifact.read_text())
        suite = json.loads(args.suite.read_text())
        validate(artifact, suite)
        binary = (args.artifact.parent / artifact["binary"]).resolve()
        if hashlib.sha256(binary.read_bytes()).hexdigest() != artifact["sha256"]:
            raise ValueError("artifact digest does not match binary")
        with tempfile.TemporaryDirectory(prefix="rubix-parity-context-") as temporary:
            context = Path(temporary)
            shutil.copyfile(binary, context / "executable")
            (context / "executable").chmod(0o755)
            shutil.copyfile(source / "driver.py", context / "driver.py")
            shutil.copyfile(args.artifact, context / "artifact.json")
            shutil.copyfile(args.suite, context / "suite.json")
            (context / "Dockerfile").write_text(
                f"FROM {BASE}\nWORKDIR /artifact\nCOPY executable driver.py artifact.json suite.json ./\n"
                "RUN mkdir /evidence && chmod 0700 /evidence\n"
                'ENTRYPOINT ["python", "/artifact/driver.py"]\n')
            hashes["Dockerfile"] = hashlib.sha256((context / "Dockerfile").read_bytes()).hexdigest()
            hashes["suite"] = hashlib.sha256(args.suite.read_bytes()).hexdigest()
            hashes["artifact_descriptor"] = hashlib.sha256(args.artifact.read_bytes()).hexdigest()
            if not command("build", ["docker", "build", "--tag", image, str(context)], timeout=600):
                raise RuntimeError("fixture image preparation failed")
        create = ["docker", "create", "--name", owned, "--network=none", "--cap-drop=ALL",
                  "--cap-add=SETUID", "--cap-add=SETGID", "--cap-add=KILL",
                  "--security-opt=no-new-privileges", "--memory=1g", "--pids-limit=128",
                  "--read-only", "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=64m,mode=1777",
                  "--mount", f"type=volume,source={volume},target=/evidence"]
        if args.inject_failure:
            create += ["--env", f"PARITY_INJECT_FAILURE={args.inject_failure}"]
        if not command("create", [*create, image]):
            raise RuntimeError("fixture container preparation failed")
        try:
            with (args.output / "container.stdout").open("wb") as out, (args.output / "container.stderr").open("wb") as err:
                status = subprocess.run(["docker", "start", "--attach", owned], stdout=out, stderr=err,
                                        timeout=600).returncode
        except subprocess.TimeoutExpired:
            command("kill-timeout", ["docker", "kill", owned])
            status = 124
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        errors.append({"operation": "prepare-or-run", "error": str(error)})
    finally:
        for kind, target, filter_value, remove in [
            ("container", owned, f"name=^/{owned}$", ["rm", "--force"]),
            ("image", image, f"reference={image}", ["image", "rm"]),
        ]:
            listing = ["docker", kind, "ls", "--all", "--filter", filter_value, "--quiet"]
            present = command(f"{kind}-inventory", listing, capture=True)
            if present and present.strip():
                if kind == "container":
                    try:
                        copy_evidence(owned, args.output)
                    except (OSError, ValueError, tarfile.TarError, subprocess.SubprocessError) as error:
                        errors.append({"operation": "evidence", "error": str(error)})
                    command("container-inspect", ["docker", "inspect", owned])
                else:
                    command("image-inspect", ["docker", "image", "inspect", image])
                command(f"{kind}-remove", ["docker", *remove, target])
            remaining = command(f"{kind}-verify-removal", listing, capture=True)
            if remaining and remaining.strip():
                errors.append({"operation": "cleanup", "error": f"owned {kind} remains"})
        volumes = command("volume-inventory", ["docker", "volume", "ls", "--filter", f"name={volume}", "--format", "{{.Name}}"], capture=True)
        if volumes and volume in volumes.splitlines():
            command("volume-inspect", ["docker", "volume", "inspect", volume])
            command("volume-remove", ["docker", "volume", "rm", volume])
        remaining_volumes = command("volume-verify-removal", ["docker", "volume", "ls", "--filter", f"name={volume}", "--format", "{{.Name}}"], capture=True)
        if remaining_volumes and volume in remaining_volumes.splitlines():
            errors.append({"operation": "cleanup", "error": "owned evidence volume remains"})
        if errors:
            status = 1
        descriptor = os.open(args.output / "runner-result.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, "w") as stream:
            stream.write(json.dumps({
                "schema_version": 1, "exit_code": status, "errors": errors, "source_sha256": hashes,
                "owned_container": owned, "owned_image": image, "owned_volume": volume,
            }, indent=2) + "\n")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
