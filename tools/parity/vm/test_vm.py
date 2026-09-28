"""Fast regression tests that do not launch a VM or alter host services."""
import hashlib
import importlib.util
import json
from unittest import mock
from pathlib import Path
import tempfile
import shlex
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

    def test_reaped_leader_with_present_group_is_not_signalled_and_keeps_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            output.mkdir()
            private = root / "vm-owned"
            private.mkdir()
            vm = mock.Mock(pid=43210, returncode=0)
            vm.poll.return_value = 0
            report = {"status": "passed", "errors": [], "cases": []}
            with mock.patch.object(adapter.os, "killpg") as signal_group:
                adapter.finish(report, output, vm, None, private, None, None, None, None)
            signal_group.assert_called_once_with(vm.pid, 0)
            result = json.loads((output / "result.json").read_text())
            self.assertEqual(result["status"], "failed")
            self.assertFalse(result.get("owned_process_group_absent", False))
            self.assertFalse(result["owned_temporary_directory_removed"])
            self.assertTrue(private.exists())
            self.assertTrue(any("no post-reap signal" in error for error in result["errors"]))

    def test_reaped_leader_with_absent_group_allows_file_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            output.mkdir()
            private = root / "vm-owned"
            private.mkdir()
            vm = mock.Mock(pid=43210, returncode=0)
            vm.poll.return_value = 0
            report = {"status": "passed", "errors": [], "cases": []}
            with mock.patch.object(adapter.os, "killpg", side_effect=ProcessLookupError) as signal_group:
                adapter.finish(report, output, vm, None, private, None, None, None, None)
            signal_group.assert_called_once_with(vm.pid, 0)
            self.assertTrue(report["owned_process_group_absent"])
            self.assertTrue(report["owned_temporary_directory_removed"])
            self.assertEqual(report["errors"], [])

    def test_failed_powerdown_does_not_signal_a_leader_reaped_before_fallback(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            vm = mock.Mock(pid=43210, returncode=0)
            vm.poll.side_effect = [None, 0, 0]
            vm.wait.return_value = 0
            report = {"status": "passed", "errors": [], "cases": []}
            with mock.patch.object(adapter.socket, "socket", side_effect=OSError("QMP unavailable")), \
                 mock.patch.object(adapter.os, "killpg", side_effect=ProcessLookupError) as signal_group:
                adapter.finish(report, output, vm, None, None, None, [], output / "qmp", mock.Mock())
            signal_group.assert_called_once_with(vm.pid, 0)
            self.assertTrue(report["owned_process_group_absent"])

    def test_unreaped_leader_still_receives_bounded_term_then_kill(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            vm = mock.Mock(pid=43210, returncode=None)
            vm.poll.side_effect = lambda: vm.returncode
            waits = 0
            def wait(timeout):
                nonlocal waits
                waits += 1
                if waits <= 2:
                    raise adapter.subprocess.TimeoutExpired("owned-qemu", timeout)
                vm.returncode = 0
                return 0
            vm.wait.side_effect = wait
            def signal_group(pid, value):
                self.assertEqual(pid, vm.pid)
                if value == 0:
                    raise ProcessLookupError
                self.assertIsNone(vm.returncode)
            report = {"status": "passed", "errors": [], "cases": []}
            with mock.patch.object(adapter.socket, "socket"), \
                 mock.patch.object(adapter.os, "killpg", side_effect=signal_group) as signals:
                adapter.finish(report, output, vm, None, None, None, [], output / "qmp", mock.Mock())
            self.assertEqual(signals.call_args_list, [
                mock.call(vm.pid, adapter.signal.SIGTERM),
                mock.call(vm.pid, adapter.signal.SIGKILL),
                mock.call(vm.pid, 0),
            ])
            self.assertTrue(report["owned_process_group_absent"])
            self.assertEqual(report["status"], "failed")

    def test_vm_uses_shared_fixture_paths_without_inline_interpolation(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            case = {"id": "file-config", "files": {"config.yaml": "kind: Config\n"},
                    "argv": ["--config", "{fixture:config.yaml}", "literal={fixture:config.yaml}"],
                    "expect": {"exit_code": 0}}
            adapter.DRIVER.validate_files(case)
            commands = []
            def command(label, argv, **kwargs):
                commands.append(shlex.split(argv[-1]))
                (output / f"{label}.stdout").write_bytes(b"")
                (output / f"{label}.stderr").write_bytes(b"")
                return 0, "", ""
            report = {"cases": []}
            adapter.run_cases([case], command, ["ssh-fixture"], output, report)
            self.assertIn("/fixtures/000/config.yaml", commands[0])
            self.assertIn("literal={fixture:config.yaml}", commands[0])
            self.assertNotIn("{fixture:config.yaml}", commands[0])
            self.assertEqual(report["cases"][0]["status"], "passed")

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
