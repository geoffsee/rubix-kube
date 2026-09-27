"""Offline provenance for the selected published Kubernetes resource bindings."""
import io
import json
import re
import subprocess
import tarfile
import tomllib

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def inspect_archive(boundary, data, contract):
    """Inspect bounded regular files in memory; never extract archive paths."""
    prefix = f"{contract['crate']}-{contract['version']}/"
    wanted = {prefix + name for name in ("Cargo.toml", ".cargo_vcs_info.json", "src/v1_35/mod.rs")}
    selected = {}
    names = set()
    total = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for item in archive:
            boundary.safe_relative(item.name)
            if not item.name.startswith(prefix) or item.name in names:
                raise boundary.InputError("unexpected or duplicate crate archive path")
            names.add(item.name)
            if not item.isfile() and not item.isdir():
                raise boundary.InputError("crate archive contains a link or special file")
            total += item.size
            if len(names) > 20000 or total > 128 * 1024 * 1024 or item.size > 8 * 1024 * 1024:
                raise boundary.InputError("crate archive exceeds inspection limits")
            if item.name in wanted:
                if not item.isfile():
                    raise boundary.InputError("crate metadata must be regular files")
                selected[item.name.removeprefix(prefix)] = archive.extractfile(item).read()
    if len(selected) != len(wanted):
        raise boundary.InputError("crate archive is missing selected metadata or v1_35 bindings")
    package = tomllib.loads(selected["Cargo.toml"].decode())
    if package["package"]["version"] != contract["version"] or package["package"]["name"] != contract["crate"]:
        raise boundary.InputError("published crate identity disagrees with contract")
    if package.get("features", {}).get(contract["feature"]) != []:
        raise boundary.InputError("published crate lacks the explicit version feature")
    vcs = json.loads(selected[".cargo_vcs_info.json"])
    if vcs.get("git", {}).get("sha1") != contract["source_commit"] or vcs.get("path_in_vcs") != "":
        raise boundary.InputError("published crate source revision disagrees with contract")
    return len(names)


def check_selection(boundary, metadata, lock, contract, checksum):
    packages = [p for p in metadata["packages"] if p["name"] == contract["crate"]]
    if len(packages) != 1:
        raise boundary.InputError("expected exactly one selected k8s-openapi package")
    package = packages[0]
    if package["version"] != contract["version"] or package.get("source") != REGISTRY:
        raise boundary.InputError("selected Kubernetes binding version/source differs")
    node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == package["id"])
    versions = {f for f in node["features"] if f.startswith("v1_") or f in ("latest", "earliest")}
    if versions != {contract["feature"]}:
        raise boundary.InputError("Kubernetes bindings require explicit v1_35 only; aliases forbidden")
    entries = [p for p in lock["package"] if p["name"] == contract["crate"]]
    if len(entries) != 1 or any(entries[0].get(k) != v for k, v in
                              {"version": contract["version"], "source": REGISTRY, "checksum": checksum}.items()):
        raise boundary.InputError("Cargo.lock does not select the verified published crate checksum")
    return sorted(node["features"])


def check_schema(boundary, data, records, contract):
    if data["k8s-openapi-schema"] != data["openapi"]:
        raise boundary.InputError("published generator schema differs from official target schema")
    version_map = data["k8s-openapi-version-map"].decode()
    match = re.search(r'SupportedVersion::V1_35\s*=>\s*"(https://[^"\n]+swagger\.json)"', version_map)
    if not match or match[1] != records["k8s-openapi-schema"]["url"]:
        raise boundary.InputError("generator version map does not select the verified schema")
    if f"/{contract['source_commit']}/" not in records["k8s-openapi-version-map"]["url"]:
        raise boundary.InputError("generator version map is not pinned to published source revision")
    if f"/{contract['schema_release']}/" not in records["k8s-openapi-schema"]["url"]:
        raise boundary.InputError("generator schema release disagrees with contract")


def check(boundary, inputs, cache, root):
    contract = inputs["kubernetes_bindings"]
    architecture = json.loads((root / "docs/architecture/upstream-inputs.json").read_text())
    if architecture["accepted_contract"]["published_kubernetes_bindings"] != {
        k: contract[k] for k in ("crate", "version", "feature")
    }:
        raise boundary.InputError("published binding selection disagrees with architecture")
    records = {r["id"]: r for r in inputs["sources"]}
    ids = ("k8s-openapi-crate", "k8s-openapi-version-map", "k8s-openapi-schema", "openapi")
    data = {key: boundary.read_verified(cache, boundary.blob_name(records[key]), records[key]) for key in ids}
    authority = next(s for s in architecture["sources"] if s["repository"] == "https://github.com/kubernetes/kubernetes")
    official = next(f for f in authority["files"] if f["path"] == records["openapi"]["path"])
    if any(official[k] != records["openapi"][k] for k in ("url", "bytes", "sha256")):
        raise boundary.InputError("official schema disagrees with architecture")
    check_schema(boundary, data, records, contract)
    count = inspect_archive(boundary, data["k8s-openapi-crate"], contract)
    # Explicit offline Cargo metadata resolves actual unified features without compiling or fetching.
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--offline", "--locked", "--format-version", "1"], cwd=root, timeout=60))
    lock_bytes = (root / "Cargo.lock").read_bytes()
    features = check_selection(boundary, metadata, tomllib.loads(lock_bytes.decode()), contract,
                               records["k8s-openapi-crate"]["sha256"])
    return {"status": "verified", "target": "published-kubernetes-bindings", "contract": contract,
            "cargo_lock_sha256": boundary.digest(lock_bytes), "resolved_features": features,
            "source_hashes": {key: records[key]["sha256"] for key in ids}, "archive_entries": count,
            "schema_definitions": len(json.loads(data["openapi"])["definitions"]),
            "limitations": "Published artifact and identical schema provenance; not generator reproduction or semantic parity."}
