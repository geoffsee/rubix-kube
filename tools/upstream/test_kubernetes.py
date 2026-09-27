"""Independent invalid artifact/selection regressions for published bindings."""
import copy
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock

import kubernetes
import upstream

CONTRACT = {"crate": "k8s-openapi", "version": "0.28.0", "feature": "v1_35",
            "source_commit": "a" * 40, "schema_release": "v1.35.6"}
PREFIX = "k8s-openapi-0.28.0/"


def archive(extra=(), revision=CONTRACT["source_commit"]):
    entries = [("Cargo.toml", b'[package]\nname="k8s-openapi"\nversion="0.28.0"\n[features]\nv1_35=[]\n'),
               (".cargo_vcs_info.json", json.dumps({"git": {"sha1": revision}, "path_in_vcs": ""}).encode()),
               ("src/v1_35/mod.rs", b"// bindings")]
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as output:
        for name, content in entries + list(extra):
            item = tarfile.TarInfo(PREFIX + name)
            if isinstance(content, bytes):
                item.size = len(content)
                output.addfile(item, io.BytesIO(content))
            else:
                item.type = content[0]
                item.linkname = "/outside"
                output.addfile(item)
    return buffer.getvalue()


class BindingTests(unittest.TestCase):
    def test_verified_archive_inspection_retains_no_extracted_files(self):
        self.assertEqual(kubernetes.inspect_archive(upstream, archive(), CONTRACT), 3)

    def test_archive_rejects_traversal_duplicates_links_and_special_files(self):
        for entry in [("../outside", b"bad"), ("Cargo.toml", b"duplicate"),
                      ("link", (tarfile.SYMTYPE,)), ("hard", (tarfile.LNKTYPE,)),
                      ("device", (tarfile.CHRTYPE,)), ("pipe", (tarfile.FIFOTYPE,))]:
            with self.subTest(entry=entry), self.assertRaises(upstream.InputError):
                kubernetes.inspect_archive(upstream, archive([entry]), CONTRACT)

    def test_published_source_revision_must_match(self):
        with self.assertRaisesRegex(upstream.InputError, "source revision"):
            kubernetes.inspect_archive(upstream, archive(revision="b" * 40), CONTRACT)

    def selection(self):
        package = {"id": "selected", "name": "k8s-openapi", "version": "0.28.0", "source": kubernetes.REGISTRY}
        metadata = {"packages": [package], "resolve": {"nodes": [{"id": "selected", "features": ["v1_35"]}]}}
        lock = {"package": [{**package, "checksum": "c" * 64}]}
        return metadata, lock

    def test_actual_resolved_selection_accepts_exact_version_feature_and_checksum(self):
        metadata, lock = self.selection()
        self.assertEqual(kubernetes.check_selection(upstream, metadata, lock, CONTRACT, "c" * 64), ["v1_35"])

    def test_feature_aliases_missing_feature_and_multiple_versions_are_rejected(self):
        for features in ([], ["latest"], ["v1_35", "latest"], ["v1_35", "v1_36"], ["earliest", "v1_35"]):
            metadata, lock = self.selection()
            metadata["resolve"]["nodes"][0]["features"] = features
            with self.subTest(features=features), self.assertRaisesRegex(upstream.InputError, "explicit v1_35"):
                kubernetes.check_selection(upstream, metadata, lock, CONTRACT, "c" * 64)

    def test_wrong_version_source_and_lock_checksum_are_rejected(self):
        for change in ("version", "source", "checksum", "duplicate"):
            metadata, lock = self.selection()
            if change == "checksum":
                lock["package"][0]["checksum"] = "d" * 64
            elif change == "duplicate":
                metadata["packages"].append(copy.deepcopy(metadata["packages"][0]))
            else:
                metadata["packages"][0][change] = "wrong"
            with self.subTest(change=change), self.assertRaises(upstream.InputError):
                kubernetes.check_selection(upstream, metadata, lock, CONTRACT, "c" * 64)

    def test_changed_schema_or_version_map_cannot_claim_identity(self):
        url = "https://raw.githubusercontent.com/kubernetes/kubernetes/v1.35.6/api/openapi-spec/swagger.json"
        data = {"openapi": b"schema", "k8s-openapi-schema": b"schema",
                "k8s-openapi-version-map": ('SupportedVersion::V1_35 => "' + url + '",').encode()}
        records = {"k8s-openapi-schema": {"url": url}, "k8s-openapi-version-map": {
            "url": "https://example.invalid/" + CONTRACT["source_commit"] + "/supported_version.rs"}}
        kubernetes.check_schema(upstream, data, records, CONTRACT)
        for field, replacement in (("k8s-openapi-schema", b"different schema"),
                                   ("k8s-openapi-version-map", b'SupportedVersion::V1_35 => "wrong",')):
            changed = {**data, field: replacement}
            with self.subTest(field=field), self.assertRaises(upstream.InputError):
                kubernetes.check_schema(upstream, changed, records, CONTRACT)

    def test_tampered_prepared_archive_fails_before_cargo_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "docs/architecture").mkdir(parents=True)
            (root / "docs/architecture/upstream-inputs.json").write_text(json.dumps({
                "accepted_contract": {"published_kubernetes_bindings": {k: CONTRACT[k] for k in ("crate", "version", "feature")}}}))
            data = archive()
            record = {"id": "k8s-openapi-crate", "sha256": upstream.digest(data), "bytes": len(data)}
            upstream.atomic_write(root, upstream.blob_name(record), data[:-1] + b"x")
            inputs = {"kubernetes_bindings": CONTRACT, "sources": [record]}
            with mock.patch.object(kubernetes.subprocess, "check_output") as cargo:
                with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
                    kubernetes.check(upstream, inputs, root, root)
                cargo.assert_not_called()


if __name__ == "__main__":
    unittest.main()
