"""Fast regression tests that do not launch a VM or alter host services."""
import hashlib
import importlib.util
import json
from unittest import mock
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("vm_adapter", Path(__file__).with_name("run.py"))
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


class VMContractTests(unittest.TestCase):
    def test_timeout_codes_cannot_become_successful_expected_exit_cases(self):
        for code in (124, 137):
            case = {"expect": {"exit_code": code}}
            self.assertEqual(adapter.ASSESS(case, code, "", ""), [])
            self.assertTrue(adapter.classify_timeout(code))
        self.assertFalse(adapter.classify_timeout(0))
        self.assertFalse(adapter.classify_timeout(1))

    def test_timeout_aborts_following_case_and_still_cleans_and_reports(self):
        for code in (124, 137):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                output = root / "output"
                output.mkdir()
                private = root / "vm-owned"
                private.mkdir()
                cases = [
                    {"id": "deadline", "argv": [], "expect": {"exit_code": code}},
                    {"id": "must-not-run", "argv": [], "expect": {"exit_code": 0}},
                ]
                calls = []
                def command(label, argv, **kwargs):
                    calls.append(label)
                    (output / f"{label}.stdout").write_bytes(b"")
                    (output / f"{label}.stderr").write_bytes(b"")
                    return code, "", ""
                report = {"status": "failed", "errors": [], "cases": []}
                adapter.run_cases(cases, command, ["ssh-fixture"], output, report)
                adapter.finish(report, output, None, None, private, None, None, None, command)
                result = json.loads((output / "result.json").read_text())
                self.assertEqual(calls, ["case-000"])
                self.assertEqual(result["unexecuted_case_ids"], ["must-not-run"])
                self.assertEqual(result["cases"][0]["status"], "failed")
                self.assertTrue(result["cases"][0]["timed_out"])
                self.assertGreaterEqual(result["cases"][0]["duration_seconds"], 0)
                self.assertTrue(result["owned_temporary_directory_removed"])
                self.assertFalse(private.exists())

    def test_cleanup_error_preserves_failed_report_and_recovery_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            output.mkdir()
            private = root / "vm-owned"
            private.mkdir()
            report = {"status": "passed", "errors": [], "cases": []}
            with mock.patch.object(adapter.shutil, "rmtree", side_effect=PermissionError("injected cleanup failure")):
                adapter.finish(report, output, None, None, private, None, None, None, None)
            result = json.loads((output / "result.json").read_text())
            self.assertEqual(result["status"], "failed")
            self.assertTrue(any("injected cleanup failure" in error for error in result["errors"]))
            self.assertFalse(result["owned_temporary_directory_removed"])
            self.assertTrue(private.exists())

    def test_dangling_cache_symlink_does_not_write_external_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            destination = root / "wheel"
            external = root / "outside-cache"
            destination.symlink_to(external)
            with self.assertRaises(ValueError):
                adapter.publish_cache_bytes(destination, b"verified", hashlib.sha256(b"verified").hexdigest())
            self.assertFalse(external.exists())
            self.assertTrue(destination.is_symlink())

    def test_bad_download_never_replaces_existing_cache_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "wheel"
            destination.write_bytes(b"old")
            with self.assertRaises(ValueError):
                adapter.publish_cache_bytes(destination, b"bad", hashlib.sha256(b"good").hexdigest())
            self.assertEqual(destination.read_bytes(), b"old")

    def test_verified_download_publishes_atomically_without_partial_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            destination = root / "wheel"
            adapter.publish_cache_bytes(destination, b"good", hashlib.sha256(b"good").hexdigest())
            self.assertEqual(destination.read_bytes(), b"good")
            self.assertEqual(list(root.iterdir()), [destination])


if __name__ == "__main__":
    unittest.main()
