#!/usr/bin/env python3
"""Source-derived semantic inventory; never reads generated Rust output."""
import argparse
import gzip
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("prepared_upstream", ROOT / "tools/upstream/upstream.py")
UPSTREAM = importlib.util.module_from_spec(spec)
spec.loader.exec_module(UPSTREAM)

# Descriptor field names/types from the pinned google/protobuf/descriptor.proto.
# Unknown fields retain wire type and value (varints are normalized integers).
FIELDS = {
    "set": {1: ("files", "file")},
    "file": {1: ("name", "text"), 2: ("package", "text"), 3: ("dependencies", "text"),
             4: ("messages", "message"), 5: ("enums", "enum"), 6: ("services", "service"),
             7: ("extensions", "field"), 8: ("options", "raw"), 12: ("syntax", "text")},
    "message": {1: ("name", "text"), 2: ("fields", "field"), 3: ("nested", "message"),
                4: ("enums", "enum"), 5: ("extension_ranges", "raw"),
                6: ("extensions", "field"), 7: ("options", "raw"),
                8: ("oneofs", "oneof"), 9: ("reserved_ranges", "raw"),
                10: ("reserved_names", "text")},
    "field": {1: ("name", "text"), 2: ("extendee", "text"), 3: ("number", "int"),
              4: ("label", "int"), 5: ("type", "int"), 6: ("type_name", "text"),
              7: ("default", "text"), 8: ("options", "raw"), 9: ("oneof_index", "int"),
              10: ("json_name", "text"), 17: ("proto3_optional", "int")},
    "enum": {1: ("name", "text"), 2: ("values", "enum_value"), 3: ("options", "raw"),
             4: ("reserved_ranges", "raw"), 5: ("reserved_names", "text")},
    "enum_value": {1: ("name", "text"), 2: ("number", "int"), 3: ("options", "raw")},
    "oneof": {1: ("name", "text"), 2: ("options", "raw")},
    "service": {1: ("name", "text"), 2: ("methods", "method"), 3: ("options", "raw")},
    "method": {1: ("name", "text"), 2: ("input", "text"), 3: ("output", "text"),
               4: ("options", "raw"), 5: ("client_streaming", "int"),
               6: ("server_streaming", "int")},
}


def strict_json(data):
    def invalid_constant(value):
        raise ValueError(f"non-JSON numeric constant: {value}")

    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid_constant)


def varint(data, pos):
    value = 0
    for shift in range(0, 70, 7):
        if pos >= len(data):
            raise ValueError("truncated varint")
        byte = data[pos]
        pos += 1
        if shift == 63 and byte > 1:
            raise ValueError("varint exceeds uint64")
        value |= (byte & 127) << shift
        if byte < 128:
            return value, pos
    raise ValueError("invalid varint")


def descriptor(data, kind="set", depth=0):
    if len(data) > 16 * 1024 * 1024 or depth > 64:
        raise ValueError("descriptor size/depth limit")
    result = {}
    pos = 0
    while pos < len(data):
        tag, pos = varint(data, pos)
        number, wire = tag >> 3, tag & 7
        if not 0 < number < (1 << 29):
            raise ValueError("invalid field number")
        if wire == 0:
            value, pos = varint(data, pos)
        elif wire in (1, 2, 5):
            if wire == 2:
                length, pos = varint(data, pos)
            else:
                length = 8 if wire == 1 else 4
            if length > len(data) - pos:
                raise ValueError("truncated field")
            value, pos = data[pos:pos + length], pos + length
        else:
            raise ValueError(f"unsupported descriptor wire type {wire}")
        name, field_kind = FIELDS[kind].get(number, (f"unknown_{number}", "unknown"))
        if field_kind == "int":
            if wire != 0:
                raise ValueError(f"wrong wire type for {kind}.{name}")
        elif field_kind != "unknown":
            if wire != 2:
                raise ValueError(f"wrong wire type for {kind}.{name}")
            if field_kind == "text":
                value = value.decode("utf-8", errors="strict")
            elif field_kind == "raw":
                value = value.hex()
            else:
                value = descriptor(value, field_kind, depth + 1)
        else:
            value = {"wire": wire, "value": value.hex() if isinstance(value, bytes) else value}
        result.setdefault(name, []).append(value)
    return result


def schema(value):
    # Preserve every keyword and property, including properties named description.
    # Prose edits are reviewable drift too; gzip keeps the full source compact.
    return value


def source_authority(record, architecture):
    matches = [(source, entry) for source in architecture["sources"]
               for entry in source.get("files", [])
               if entry.get("url") == record["url"]]
    if len(matches) != 1:
        raise ValueError(f"source missing/ambiguous in architecture contract: {record['id']}")
    authority, matching = matches[0]
    if any(record[field] != matching[field] for field in ("sha256", "bytes", "url")):
        raise ValueError(f"architecture and prepared-input contracts disagree: {record['id']}")
    return {"repository": authority["repository"], "commit": authority["commit"]}


def containerd_descriptors(inputs, cache, platform, compiler):
    records = [record for record in inputs["sources"] if record.get("target") == "containerd"]
    if not records:
        raise ValueError("missing containerd schema inputs; use finalized runtime input manifest")
    with tempfile.TemporaryDirectory(prefix="rubix-containerd-semantic-") as temporary:
        work = Path(temporary)
        names = set()
        for record in records:
            name = record["proto_path"]
            if name in names:
                raise ValueError(f"duplicate protobuf source: {name}")
            names.add(name)
            UPSTREAM.atomic_write(work, name, UPSTREAM.read_verified(cache, UPSTREAM.blob_name(record), record))
        imports = []
        for record in UPSTREAM.compiler_record(inputs, platform)["files"]:
            if record["path"].startswith("include/"):
                name = record["path"].removeprefix("include/")
                if name in names:
                    raise ValueError(f"protobuf import shadows source: {name}")
                names.add(name)
                imports.append(record)
                UPSTREAM.atomic_write(work, name, UPSTREAM.read_verified(cache, f"protoc/{platform}/{record['path']}", record))
        # Only this private, fully verified graph is on protoc's import path.
        # Missing/public/weak imports are parsed by protoc itself, not regex.
        subprocess.run([str(compiler), f"--proto_path={work}", "--include_imports",
                        f"--descriptor_set_out={work / 'containerd.pb'}",
                        *[record["proto_path"] for record in records]],
                       check=True, timeout=30, capture_output=True)
        result = descriptor((work / "containerd.pb").read_bytes())
        actual = {file["name"][0] for file in result["files"]}
        if not actual.issubset(names) or not {r["proto_path"] for r in records}.issubset(actual):
            raise ValueError("unexpected descriptor source inventory")
        return result, records, imports


def extract(cache, input_manifest=UPSTREAM.MANIFEST, architecture_manifest=ROOT / "docs/architecture/upstream-inputs.json"):
    inputs = UPSTREAM.manifest(input_manifest)
    platform = UPSTREAM.host_platform()
    compiler = UPSTREAM.verify(inputs, cache, platform)
    architecture = strict_json(architecture_manifest.read_bytes())
    records = {r["id"]: r for r in inputs["sources"]}
    for key in ("cri", "openapi"):
        source_authority(records[key], architecture)
    with tempfile.TemporaryDirectory(prefix="rubix-semantic-") as temporary:
        work = Path(temporary)
        (work / "api.proto").write_bytes(UPSTREAM.read_verified(cache, UPSTREAM.blob_name(records["cri"]), records["cri"]))
        subprocess.run([str(compiler), f"--proto_path={work}", f"--descriptor_set_out={work / 'api.pb'}", "api.proto"], check=True, timeout=30, capture_output=True)
        protocol = descriptor((work / "api.pb").read_bytes())
    document = strict_json(UPSTREAM.read_verified(cache, UPSTREAM.blob_name(records["openapi"]), records["openapi"]))
    if document.get("swagger") != "2.0" or not isinstance(document.get("definitions"), dict):
        raise ValueError("unsupported OpenAPI format")
    containerd, containerd_records, imports = containerd_descriptors(inputs, cache, platform, compiler)
    authorities = {record["id"]: source_authority(record, architecture) for record in containerd_records}
    return {"schema_version": 2, "authority": source_authority(records["cri"], architecture),
            "inputs": {key: records[key] for key in ("cri", "openapi")},
            "protoc_version": inputs["protoc_version"], "cri": protocol,
            "openapi_definitions": schema(document["definitions"]),
            "containerd": containerd, "containerd_authorities": authorities,
            "containerd_inputs": containerd_records, "well_known_imports": imports}


def changes(before, after, path=""):
    if type(before) is not type(after):
        yield {"path": path, "before": before, "after": after}
    elif isinstance(before, dict):
        for key in sorted(before.keys() | after.keys()):
            child = path + "/" + key.replace("~", "~0").replace("/", "~1")
            if key not in before:
                yield {"path": child, "added": after[key]}
            elif key not in after:
                yield {"path": child, "removed": before[key]}
            else:
                yield from changes(before[key], after[key], child)
    elif isinstance(before, list) and isinstance(after, list) and len(before) == len(after):
        for index, (old, new) in enumerate(zip(before, after)):
            yield from changes(old, new, path + f"/{index}")
    elif before != after:
        yield {"path": path, "before": before, "after": after}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "extract"))
    parser.add_argument("--cache-dir", type=Path, default=ROOT / "target/upstream")
    parser.add_argument("--inventory", type=Path, default=HERE / "inventory.json.gz")
    parser.add_argument("--input-manifest", type=Path, default=UPSTREAM.MANIFEST)
    parser.add_argument("--architecture-manifest", type=Path, default=ROOT / "docs/architecture/upstream-inputs.json")
    args = parser.parse_args()
    current = extract(args.cache_dir.resolve(), args.input_manifest, args.architecture_manifest)
    if args.command == "extract":
        # Explicit refresh only; check never rewrites the accepted inventory.
        UPSTREAM.atomic_write(args.inventory.parent, args.inventory.name,
                              gzip.compress(json.dumps(current, sort_keys=True, separators=(",", ":")).encode(), mtime=0))
        return 0
    accepted = strict_json(gzip.decompress(args.inventory.read_bytes()))
    diff = list(changes(accepted, current))
    print(json.dumps({"status": "drift" if diff else "unchanged", "changes": diff}, indent=2))
    return 1 if diff else 0


if __name__ == "__main__":
    raise SystemExit(main())
