"""Negative coverage for the explicit upstream preparation boundary."""

import io
import json
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

import upstream


def record(data, **extra):
    return {"bytes": len(data), "sha256": upstream.digest(data), **extra}


def zip_bytes(entries):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, content in entries:
            archive.writestr(name, content)
    return buffer.getvalue()


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.cache = self.root / "cache"
        self.proto = b'syntax = "proto3";\n'
        self.binary = b"#!/bin/sh\nprintf 'libprotoc 36.2\\n'\n"
        self.archive = zip_bytes([("bin/protoc", self.binary)])
        self.source = record(self.proto, id="cri", url="https://example.invalid/api.proto")
        self.compiler = record(self.archive, platform=upstream.host_platform(),
                               url="https://example.invalid/compiler.zip",
                               files=[record(self.binary, path="bin/protoc")])
        self.inputs = {"schema_version": 1, "protoc_version": "36.2",
                       "sources": [self.source], "protoc_archives": [self.compiler]}

    def prepare(self):
        with mock.patch.object(upstream, "download", side_effect=[self.proto, self.archive]):
            upstream.fetch(self.inputs, self.cache, upstream.host_platform())

    def test_valid_preparation_verifies_without_network(self):
        self.prepare()
        with mock.patch.object(upstream.urllib.request, "urlopen", side_effect=AssertionError("network")):
            compiler = upstream.verify(self.inputs, self.cache, upstream.host_platform())
        self.assertEqual(compiler.read_bytes(), self.binary)

    def test_missing_input_is_a_failure_with_explicit_fetch_instruction(self):
        with self.assertRaisesRegex(upstream.InputError, "missing prepared input.*run fetch"):
            upstream.verify(self.inputs, self.cache, upstream.host_platform())

    def test_corruption_is_rejected_before_using_compiler(self):
        self.prepare()
        (self.cache / upstream.blob_name(self.source)).write_bytes(b"corruption")
        with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
            upstream.verify(self.inputs, self.cache, upstream.host_platform())

    def test_compiler_with_matching_version_but_different_content_is_rejected(self):
        self.prepare()
        alternate = self.root / "alternate-protoc"
        alternate.write_bytes(self.binary + b"# different executable\n")
        with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
            upstream.verify(self.inputs, self.cache, upstream.host_platform(), alternate)

    def test_modified_prepared_compiler_is_rejected_before_execution(self):
        self.prepare()
        compiler = self.cache / f"protoc/{upstream.host_platform()}/bin/protoc"
        compiler.write_bytes(self.binary + b"# tampered\n")
        with mock.patch.object(upstream.subprocess, "check_output") as execute:
            with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
                upstream.generate(self.inputs, self.cache, upstream.host_platform(),
                                  self.root / "generator", self.root / "out")
            execute.assert_not_called()

    def test_tampered_archive_rejected_even_when_extracted_compiler_is_valid(self):
        self.prepare()
        (self.cache / upstream.blob_name(self.compiler)).write_bytes(b"invalid ZIP")
        with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
            upstream.verify(self.inputs, self.cache, upstream.host_platform())

    def test_download_rejects_wrong_length_and_does_not_commit_input(self):
        response = mock.MagicMock()
        response.__enter__.return_value = response
        response.url = self.source["url"]
        response.read.return_value = b"incorrect"
        with mock.patch.object(upstream.urllib.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
                upstream.fetch(self.inputs, self.cache, upstream.host_platform())
        self.assertFalse((self.cache / upstream.blob_name(self.source)).exists())

    def test_existing_valid_input_is_preserved_when_later_fetch_fails(self):
        upstream.atomic_write(self.cache, upstream.blob_name(self.source), self.proto)
        with mock.patch.object(upstream, "download", side_effect=OSError("offline")):
            with self.assertRaisesRegex(OSError, "offline"):
                upstream.fetch(self.inputs, self.cache, upstream.host_platform())
        self.assertEqual((self.cache / upstream.blob_name(self.source)).read_bytes(), self.proto)

    def test_zip_path_traversal_is_rejected_before_extraction(self):
        raw = zip_bytes([("../outside", b"bad"), ("bin/protoc", self.binary)])
        archive = record(raw, files=self.compiler["files"])
        with self.assertRaisesRegex(upstream.InputError, "unsafe relative path"):
            upstream.archive_members(raw, archive)
        self.assertFalse((self.root / "outside").exists())

    def test_zip_symlink_is_rejected_before_extraction(self):
        link = zipfile.ZipInfo("bin/protoc")
        link.create_system = 3
        link.external_attr = (stat.S_IFLNK | 0o777) << 16
        raw = zip_bytes([(link, b"/outside")])
        with self.assertRaisesRegex(upstream.InputError, "unsupported ZIP member type"):
            upstream.archive_members(raw, record(raw, files=[]))

    def test_cache_symlink_cannot_overwrite_outside_selected_directory(self):
        self.cache.mkdir()
        outside = self.root / "outside"
        outside.mkdir()
        (self.cache / "blobs").symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(upstream.InputError, "symlink inside"):
            upstream.atomic_write(self.cache, upstream.blob_name(self.source), self.proto)
        self.assertEqual(list(outside.iterdir()), [])

    def test_relative_output_cannot_escape(self):
        with self.assertRaisesRegex(upstream.InputError, "unsafe relative path"):
            upstream.atomic_write(self.cache, "../outside", b"bad")

    def test_unsupported_platform_fails_before_network(self):
        with mock.patch.object(upstream, "download") as download:
            with self.assertRaisesRegex(upstream.InputError, "no locked protoc"):
                upstream.fetch(self.inputs, self.cache, "unsupported-cpu")
            download.assert_not_called()

    def test_drift_check_preserves_committed_output(self):
        self.prepare()
        output = self.root / "output"
        output.mkdir()
        (output / "runtime.v1.rs").write_bytes(b"stale")
        generator = self.root / "generator"
        generator.write_bytes(b"fixture generator identity")

        def generate(arguments, **_kwargs):
            generated = Path(arguments[-1])
            generated.mkdir()
            (generated / "runtime.v1.rs").write_bytes(b"fresh")

        with mock.patch.object(upstream.subprocess, "check_output", return_value="libprotoc 36.2\n"):
            with mock.patch.object(upstream.subprocess, "run", side_effect=generate):
                with self.assertRaisesRegex(upstream.InputError, "generated output drift"):
                    upstream.generate(self.inputs, self.cache, upstream.host_platform(), generator, output, check=True)
        self.assertEqual((output / "runtime.v1.rs").read_bytes(), b"stale")

    def test_committed_manifest_has_no_unlocked_compiler_members(self):
        inputs = upstream.manifest()
        for compiler in inputs["protoc_archives"]:
            self.assertEqual(len(compiler["files"]), 6)
            self.assertEqual(compiler["files"][0]["path"], "bin/protoc")
        # Parseable JSON remains useful to independent verification tooling.
        self.assertEqual(json.loads(upstream.MANIFEST.read_text())["schema_version"], 1)

    def add_containerd_proto(self, name, content):
        item = record(content, id=f"containerd:{name}", target="containerd", proto_path=name)
        self.inputs["sources"].append(item)
        upstream.atomic_write(self.cache, upstream.blob_name(item), content)
        return item

    def test_missing_transitive_import_fails_before_any_staged_input_is_written(self):
        self.prepare()
        self.add_containerd_proto("example/service.proto", b'import "example/missing.proto";\n')
        destination = self.root / "prepared"
        with self.assertRaisesRegex(upstream.InputError, "missing locked protobuf import"):
            upstream.prepare_containerd(self.inputs, self.cache, upstream.host_platform(), destination)
        self.assertFalse(destination.exists())

    def test_corrupted_transitive_import_is_rejected(self):
        self.prepare()
        self.add_containerd_proto("example/service.proto", b'import "example/types.proto";\n')
        item = self.add_containerd_proto("example/types.proto", b'message Value {}\n')
        (self.cache / upstream.blob_name(item)).write_bytes(b"corrupted import")
        with self.assertRaisesRegex(upstream.InputError, "SHA-256 mismatch"):
            upstream.prepare_containerd(self.inputs, self.cache, upstream.host_platform(), self.root / "prepared")

    def test_duplicate_import_name_is_rejected_instead_of_shadowing_verified_source(self):
        self.prepare()
        self.add_containerd_proto("example/types.proto", b'message Original {}\n')
        self.add_containerd_proto("example/types.proto", b'message Replacement {}\n')
        with self.assertRaisesRegex(upstream.InputError, "duplicate protobuf import name"):
            upstream.prepare_containerd(self.inputs, self.cache, upstream.host_platform(), self.root / "prepared")

    def test_protocol_coverage_includes_streams_but_ignores_comments(self):
        proto = b'''package containerd.services.content.v1;
// service Fake { rpc Imaginary(Request) returns (Response); }
service Content {
  // rpc Invented(Request) returns (Response);
  rpc Write(stream Request) returns (stream Response);
  rpc Read(Request) returns (stream Response);
}
'''
        self.assertEqual(upstream.protocol_routes(proto), [
            b"/containerd.services.content.v1.Content/Write",
            b"/containerd.services.content.v1.Content/Read",
        ])

    def test_missing_generated_rpc_fails_before_replacing_previous_output(self):
        self.prepare()
        proto = b'''package example;
service Storage {
  rpc Write(stream Request) returns (stream Response);
}
'''
        self.add_containerd_proto("example/service.proto", proto)
        self.inputs["containerd_output_files"] = ["example.rs"]
        output = self.root / "output"
        output.mkdir()
        (output / "example.rs").write_bytes(b"previous valid generation")
        generator = self.root / "generator"
        generator.write_bytes(b"fixture generator")

        def generate(arguments, **_kwargs):
            generated = Path(arguments[-1])
            generated.mkdir()
            (generated / "example.rs").write_bytes(b"output without required Write RPC")

        with mock.patch.object(upstream.subprocess, "check_output", return_value="libprotoc 36.2\n"):
            with mock.patch.object(upstream.subprocess, "run", side_effect=generate):
                with self.assertRaisesRegex(upstream.InputError, "missing RPC /example.Storage/Write"):
                    upstream.generate(self.inputs, self.cache, upstream.host_platform(), generator, output,
                                      target="containerd")
        self.assertEqual((output / "example.rs").read_bytes(), b"previous valid generation")

    def test_nonregular_later_destination_preserves_all_previous_outputs(self):
        self.prepare()
        self.add_containerd_proto("example/types.proto", b'package example; message Value {}')
        self.inputs["containerd_output_files"] = ["a.rs", "b.rs", "z.rs"]
        output = self.root / "output"
        output.mkdir()
        previous = {"a.rs": b"previous A", "b.rs": b"previous B"}
        for name, content in previous.items():
            (output / name).write_bytes(content)
        (output / "z.rs").mkdir()
        (output / "z.rs" / "retained").write_bytes(b"directory content")
        generator = self.root / "generator"
        generator.write_bytes(b"fixture generator")

        def generate(arguments, **_kwargs):
            generated = Path(arguments[-1])
            generated.mkdir()
            for name in self.inputs["containerd_output_files"]:
                (generated / name).write_bytes(b"replacement")

        with mock.patch.object(upstream.subprocess, "check_output", return_value="libprotoc 36.2\n"), \
             mock.patch.object(upstream.subprocess, "run", side_effect=generate):
            with self.assertRaisesRegex(upstream.InputError, "not a regular file"):
                upstream.generate(self.inputs, self.cache, upstream.host_platform(), generator, output,
                                  target="containerd")
        for name, content in previous.items():
            self.assertEqual((output / name).read_bytes(), content)
        self.assertEqual((output / "z.rs" / "retained").read_bytes(), b"directory content")

    def test_generator_timeout_preserves_output_and_removes_partial_temporary_files(self):
        self.prepare()
        output = self.root / "output"
        output.mkdir()
        committed = output / "runtime.v1.rs"
        committed.write_bytes(b"previous valid generation")
        generator = self.root / "generator"
        generator.write_bytes(b"fixture generator identity")
        temporary_outputs = []

        def time_out(arguments, **kwargs):
            self.assertEqual(kwargs["timeout"], 120)
            generated = Path(arguments[-1])
            generated.mkdir()
            (generated / "runtime.v1.rs").write_bytes(b"incomplete generation")
            temporary_outputs.append(generated)
            raise subprocess.TimeoutExpired(arguments, kwargs["timeout"])

        with mock.patch.object(upstream.subprocess, "check_output", return_value="libprotoc 36.2\n"):
            with mock.patch.object(upstream.subprocess, "run", side_effect=time_out):
                with self.assertRaises(subprocess.TimeoutExpired):
                    upstream.generate(self.inputs, self.cache, upstream.host_platform(), generator, output)
        self.assertEqual(committed.read_bytes(), b"previous valid generation")
        self.assertTrue(temporary_outputs)
        self.assertFalse(temporary_outputs[0].exists())

    def test_compiler_probe_timeout_prevents_generation_and_preserves_output(self):
        self.prepare()
        output = self.root / "output"
        output.mkdir()
        committed = output / "runtime.v1.rs"
        committed.write_bytes(b"previous valid generation")

        def time_out(arguments, **kwargs):
            self.assertEqual(kwargs["timeout"], 10)
            raise subprocess.TimeoutExpired(arguments, kwargs["timeout"])

        with mock.patch.object(upstream.subprocess, "check_output", side_effect=time_out):
            with mock.patch.object(upstream.subprocess, "run") as generate:
                with self.assertRaises(subprocess.TimeoutExpired):
                    upstream.generate(self.inputs, self.cache, upstream.host_platform(),
                                      self.root / "generator", output)
                generate.assert_not_called()
        self.assertEqual(committed.read_bytes(), b"previous valid generation")


if __name__ == "__main__":
    unittest.main()
