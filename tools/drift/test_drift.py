"""Independent descriptor bytes and source mutations, without generated Rust."""
import copy
import gzip
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import drift


class InventoryTests(unittest.TestCase):
    def test_duplicate_schema_key_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            drift.strict_json('{"type":"string","type":"integer"}')

    def test_non_json_numeric_constants_fail_closed(self):
        for constant in ("NaN", "Infinity", "-Infinity"):
            with self.subTest(constant=constant), self.assertRaisesRegex(ValueError, "non-JSON numeric"):
                drift.strict_json('{"default":' + constant + '}')

    def test_cli_check_fails_drift_without_rewriting_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "accepted.json.gz"
            original = gzip.compress(b'{"field_type":"string"}', mtime=0)
            path.write_bytes(original)
            with mock.patch("sys.argv", ["drift", "check", "--inventory", str(path)]), \
                 mock.patch.object(drift, "extract", return_value={"field_type": "integer"}), \
                 mock.patch("builtins.print") as output:
                self.assertEqual(drift.main(), 1)
                self.assertEqual(json.loads(output.call_args.args[0])["changes"][0]["path"], "/field_type")
            self.assertEqual(path.read_bytes(), original)

    def test_hand_encoded_field_number_and_type(self):
        # FieldDescriptorProto: name=x, number=7, label=optional, type=int32.
        self.assertEqual(drift.descriptor(bytes.fromhex("0a0178180720012805"), "field"),
                         {"name": ["x"], "number": [7], "label": [1], "type": [5]})

    def test_unknown_descriptor_field_is_preserved_and_changes_detected(self):
        original = drift.descriptor(bytes.fromhex("980607"), "field")
        self.assertEqual(original, {"unknown_99": [{"wire": 0, "value": 7}]})
        changed = drift.descriptor(bytes.fromhex("980608"), "field")
        self.assertTrue(list(drift.changes(original, changed)))

    def test_malformed_descriptor_is_rejected(self):
        for raw in (b"\x0a\x05x", b"\x00", b"\x0b", b"\x08\x01", b"\x80" * 11):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                drift.descriptor(raw, "field")

    def test_openapi_retains_property_named_description_and_unknown_extensions(self):
        source = {"properties": {"description": {"type": "string"}}, "x-new-rule": 12}
        self.assertEqual(drift.schema(source), source)
        changed = copy.deepcopy(source)
        changed["properties"]["description"]["type"] = "integer"
        self.assertTrue(list(drift.changes(source, changed)))

    def test_frozen_source_has_known_streaming_rpc_and_field(self):
        inventory = json.loads(gzip.decompress((Path(__file__).parent / "inventory.json.gz").read_bytes()))
        file = inventory["cri"]["files"][0]
        runtime = next(s for s in file["services"] if s["name"] == ["RuntimeService"])
        event = next(m for m in runtime["methods"] if m["name"] == ["GetContainerEvents"])
        self.assertEqual(event["input"], [".runtime.v1.GetEventsRequest"])
        self.assertEqual(event["output"], [".runtime.v1.ContainerEventResponse"])
        self.assertEqual(event["server_streaming"], [1])
        message = next(m for m in file["messages"] if m["name"] == ["VersionRequest"])
        self.assertEqual(message["fields"][0]["number"], [1])
        self.assertEqual(message["fields"][0]["type"], [9])  # TYPE_STRING

    def test_containerd_inventory_has_full_selected_graph_and_independent_stat_zero(self):
        inventory = json.loads(gzip.decompress((Path(__file__).parent / "inventory.json.gz").read_bytes()))
        files = inventory["containerd"]["files"]
        self.assertEqual(len(inventory["containerd_inputs"]), 17)
        services = [service for file in files for service in file.get("services", [])]
        self.assertEqual(len(services), 11)
        self.assertEqual(sum(len(service["methods"]) for service in services), 65)
        content = next(file for file in files if file["package"] == ["containerd.services.content.v1"])
        action = next(enum for enum in content["enums"] if enum["name"] == ["WriteAction"])
        # Independently encoded EnumValueDescriptorProto(name=STAT, number=0).
        self.assertEqual(action["values"][0], drift.descriptor(b"\x0a\x04STAT\x10\x00", "enum_value"))
        write = next(method for method in content["services"][0]["methods"] if method["name"] == ["Write"])
        self.assertEqual(write["client_streaming"], [1])
        self.assertEqual(write["server_streaming"], [1])
        changed = copy.deepcopy(inventory["containerd"])
        changed["files"].remove(next(file for file in changed["files"] if file["package"] == ["containerd.services.version.v1"]))
        self.assertTrue(list(drift.changes(inventory["containerd"], changed)))

    def test_source_authority_rejects_missing_or_modified_containerd_pin(self):
        record = {"id": "version", "url": "https://example.invalid/version.proto", "sha256": "abc", "bytes": 3}
        architecture = {"sources": [{"repository": "official", "commit": "pin", "files": [dict(record)]}]}
        self.assertEqual(drift.source_authority(record, architecture)["commit"], "pin")
        with self.assertRaisesRegex(ValueError, "disagree"):
            drift.source_authority(dict(record, bytes=4), architecture)
        with self.assertRaisesRegex(ValueError, "missing/ambiguous"):
            drift.source_authority(record, {"sources": []})

    def test_schema_type_required_enum_and_removal_changes_are_visible(self):
        before = {"Thing": {"properties": {"x": {"type": "string", "enum": ["a"]}}, "required": ["x"]}}
        for after in ({}, {"Thing": {"required": []}},
                      {"Thing": {"properties": {"x": {"type": "integer", "enum": ["b"]}}, "required": ["x"]}}):
            self.assertTrue(list(drift.changes(before, after)))

    def test_real_parser_detects_source_rpc_and_field_mutations(self):
        cache = drift.ROOT / "target/upstream"
        try:
            compiler = drift.UPSTREAM.verify(drift.UPSTREAM.manifest(), cache, drift.UPSTREAM.host_platform())
        except (OSError, drift.UPSTREAM.InputError) as error:
            self.skipTest(f"explicit prepared protoc needed for parser integration: {error}")
        source = 'syntax="proto3"; package fixture; message A { string x = 1; } service S { rpc Read(A) returns(A); }'

        def parse(text):
            with tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary)
                (path / "test.proto").write_text(text)
                subprocess.run([str(compiler), f"-I{path}", f"--descriptor_set_out={path / 'out'}", "test.proto"], check=True, capture_output=True, timeout=10)
                return drift.descriptor((path / "out").read_bytes())

        accepted = parse(source)
        for replacement in (source.replace("x = 1", "x = 2"), source.replace("string x", "int32 x"),
                            source.replace("rpc Read", "rpc Write"), source.replace("returns(A)", "returns(stream A)")):
            # Both inputs are freshly parsed; generated-code freshness cannot hide drift.
            self.assertTrue(list(drift.changes(accepted, parse(replacement))))


if __name__ == "__main__":
    unittest.main()
