#!/usr/bin/env python3
"""Prepare pinned inputs explicitly; verify and generate CRI clients offline."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import stat
import subprocess
import sys
import tempfile
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path(__file__).with_name("inputs.json")


class InputError(Exception):
    """An input does not satisfy the committed preparation contract."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate_record(record):
    if not re.fullmatch(r"[0-9a-f]{64}", record["sha256"]):
        raise InputError("invalid SHA-256 in input manifest")
    if not isinstance(record["bytes"], int) or record["bytes"] <= 0:
        raise InputError("invalid input length in manifest")


def validate_bytes(data, record, label):
    if len(data) != record["bytes"] or digest(data) != record["sha256"]:
        raise InputError(f"length or SHA-256 mismatch: {label}")


def safe_relative(name):
    parts = PurePosixPath(name).parts
    if not parts or name.startswith("/") or "\\" in name or any(p in ("..", ".") for p in parts):
        raise InputError(f"unsafe relative path: {name}")
    # PurePath normalizes '.', so reject the original spelling too.
    if any(p in (".", "..", "") for p in name.rstrip("/").split("/")):
        raise InputError(f"unsafe relative path: {name}")
    return parts


def owned_path(root, relative):
    """Refuse symlinks below the caller-selected root, including dangling links."""
    path = root
    for part in safe_relative(relative):
        path = path / part
        if path.is_symlink():
            raise InputError(f"symlink inside selected directory: {path}")
    return path


def atomic_write(root, relative, data, executable=False):
    path = owned_path(root, relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    # Recheck after creation; do not follow an existing cache entry's symlink.
    path = owned_path(root, relative)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".prepare-", delete=False) as target:
            temporary = Path(target.name)
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        temporary.chmod(0o755 if executable else 0o644)
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def blob_name(record):
    return f"blobs/{record['sha256']}"


def read_verified(root, relative, record):
    path = owned_path(root, relative)
    try:
        data = path.read_bytes()
    except (FileNotFoundError, IsADirectoryError) as error:
        raise InputError(f"missing prepared input: {path}; run fetch explicitly") from error
    validate_bytes(data, record, path)
    return data


def download(record):
    if not record["url"].startswith("https://"):
        raise InputError("input downloads require HTTPS")
    request = urllib.request.Request(record["url"], headers={"User-Agent": "rubix-upstream/1"})
    with urllib.request.urlopen(request, timeout=60) as response:
        if not response.url.startswith("https://"):
            raise InputError("download redirected away from HTTPS")
        data = response.read(record["bytes"] + 1)
    validate_bytes(data, record, record["url"])
    return data


def manifest(path=MANIFEST):
    value = json.loads(path.read_text())
    if value["schema_version"] != 1:
        raise InputError("unsupported input manifest version")
    for record in value["sources"] + value["protoc_archives"]:
        validate_record(record)
    for archive in value["protoc_archives"]:
        for record in archive["files"]:
            safe_relative(record["path"])
            validate_record(record)
    return value


def host_platform():
    machine = {"aarch64": "arm64", "arm64": "arm64", "x86_64": "amd64"}.get(platform.machine())
    return f"{platform.system().lower()}-{machine}"


def compiler_record(inputs, selected):
    for record in inputs["protoc_archives"]:
        if record["platform"] == selected:
            return record
    raise InputError(f"no locked protoc archive for platform {selected}")


def archive_members(data, record):
    """Validate the entire ZIP namespace, then read only explicitly locked members."""
    validate_bytes(data, record, "protoc archive")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = set()
        for item in archive.infolist():
            safe_relative(item.filename)
            if item.filename in names:
                raise InputError(f"duplicate ZIP member: {item.filename}")
            names.add(item.filename)
            mode = item.external_attr >> 16
            if stat.S_ISLNK(mode) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)):
                raise InputError(f"unsupported ZIP member type: {item.filename}")
        result = {}
        for item in record["files"]:
            try:
                info = archive.getinfo(item["path"])
            except KeyError as error:
                raise InputError(f"missing locked ZIP member: {item['path']}") from error
            if info.file_size != item["bytes"]:
                raise InputError(f"wrong ZIP member size: {item['path']}")
            content = archive.read(info)
            validate_bytes(content, item, item["path"])
            result[item["path"]] = content
        return result


def fetch(inputs, cache, selected):
    compiler = compiler_record(inputs, selected)
    for record in inputs["sources"] + [compiler]:
        path = owned_path(cache, blob_name(record))
        if path.exists():
            read_verified(cache, blob_name(record), record)
        else:
            atomic_write(cache, blob_name(record), download(record))
    members = archive_members(read_verified(cache, blob_name(compiler), compiler), compiler)
    for name, data in members.items():
        atomic_write(cache, f"protoc/{selected}/{name}", data, executable=name == "bin/protoc")
    return verify(inputs, cache, selected)


def verify(inputs, cache, selected, alternate_protoc=None):
    for record in inputs["sources"]:
        read_verified(cache, blob_name(record), record)
    compiler = compiler_record(inputs, selected)
    archive_members(read_verified(cache, blob_name(compiler), compiler), compiler)
    for record in compiler["files"]:
        read_verified(cache, f"protoc/{selected}/{record['path']}", record)
    binary_record = next(r for r in compiler["files"] if r["path"] == "bin/protoc")
    protoc = owned_path(cache, f"protoc/{selected}/bin/protoc")
    if alternate_protoc is not None:
        protoc = alternate_protoc.resolve(strict=True)
        validate_bytes(protoc.read_bytes(), binary_record, protoc)
    return protoc


def generate(inputs, cache, selected, generator, output, check=False, alternate_protoc=None):
    if selected != host_platform():
        raise InputError("generation requires the locked compiler for the current host platform")
    protoc = verify(inputs, cache, selected, alternate_protoc)
    version = subprocess.check_output([str(protoc), "--version"], text=True, timeout=10).strip()
    if version != f"libprotoc {inputs['protoc_version']}":
        raise InputError(f"unexpected verified compiler version: {version}")
    if not generator.is_file():
        raise InputError(f"generator missing: {generator}; build rubix-upstream-codegen explicitly")
    source = next(r for r in inputs["sources"] if r["id"] == "cri")
    proto = read_verified(cache, blob_name(source), source)
    with tempfile.TemporaryDirectory(prefix="rubix-cri-generation-") as temporary:
        work = Path(temporary)
        (work / "api.proto").write_bytes(proto)
        generated = work / "generated"
        subprocess.run([str(generator), str(work / "api.proto"), str(protoc), str(generated)],
                       check=True, timeout=120)
        expected = {"runtime.v1.rs"}
        if {p.name for p in generated.iterdir()} != expected:
            raise InputError("generator returned an unexpected output inventory")
        content = (generated / "runtime.v1.rs").read_bytes()
        methods = re.findall(rb"rpc\s+(\w+)\(", proto)
        for method in methods:
            if b"/runtime.v1.RuntimeService/" + method not in content and b"/runtime.v1.ImageService/" + method not in content:
                raise InputError(f"generated client is missing RPC {method.decode()}")
        if check:
            path = owned_path(output, "runtime.v1.rs")
            if not path.is_file() or path.read_bytes() != content:
                raise InputError(f"generated output drift: {path}")
            if {p.name for p in output.iterdir()} != expected:
                raise InputError(f"unexpected files in generated output: {output}")
        else:
            atomic_write(output, "runtime.v1.rs", content)
    return {
        "schema_version": 1,
        "mode": "check" if check else "generate",
        "platform": selected,
        "source_sha256": source["sha256"],
        "compiler_sha256": digest(protoc.read_bytes()),
        "generator_sha256": digest(generator.read_bytes()),
        "cargo_lock_sha256": digest((ROOT / "Cargo.lock").read_bytes()),
        "output_sha256": digest(content),
        "output_bytes": len(content),
        "rpc_count": len(methods),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["fetch", "verify", "generate-cri", "check-cri"])
    parser.add_argument("--cache-dir", type=Path, default=ROOT / "target/upstream")
    parser.add_argument("--platform", default=host_platform())
    parser.add_argument("--protoc", type=Path, help="optional compiler, must match the locked binary digest")
    parser.add_argument("--generator", type=Path, default=ROOT / "target/debug/rubix-upstream-codegen")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "crates/rubix-cri/src/generated")
    args = parser.parse_args(argv)
    try:
        inputs = manifest()
        cache = args.cache_dir.resolve()
        if args.command == "fetch":
            fetch(inputs, cache, args.platform)
            result = {"status": "prepared", "platform": args.platform}
        elif args.command == "verify":
            verify(inputs, cache, args.platform, args.protoc)
            result = {"status": "verified", "platform": args.platform}
        else:
            result = generate(inputs, cache, args.platform, args.generator.resolve(), args.output_dir.resolve(),
                              check=args.command == "check-cri", alternate_protoc=args.protoc)
        result["input_manifest_sha256"] = digest(MANIFEST.read_bytes())
        print(json.dumps(result, sort_keys=True))
        return 0
    except (InputError, OSError, ValueError, KeyError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        print(f"upstream: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
