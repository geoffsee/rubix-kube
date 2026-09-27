"""Fast regression tests for distribution-neutral expectations and input validation."""
import copy
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

from driver import assess, execute
from run import publish_evidence, validate


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.case = {"id": "example", "argv": ["--version"],
                     "expect": {"exit_code": 0, "stdout_contains": ["version"], "stderr_equals": ""}}
        self.suite = {"schema_version": 1, "id": "test", "cases": [self.case]}
        self.artifact = {"schema_version": 1, "kind": "go", "binary": "node", "sha256": "a" * 64,
                         "version": "test", "source": {"revision": "b" * 40, "repository": "example"}}

    def test_same_expectations_apply_to_both_distributions(self):
        for kind in ("go", "rust"):
            artifact = {**self.artifact, "kind": kind}
            validate(artifact, self.suite)
            self.assertEqual(assess(self.case, 0, "version test", ""), [])

    def test_broken_fixture_and_wrong_exit_both_fail(self):
        self.assertEqual(len(assess(self.case, 1, "unexpected", "")), 2)

    def test_privileged_case_is_a_gap_without_starting_a_process(self):
        self.case["privilege"] = "privileged"
        result = execute(self.case, 0, None)
        self.assertEqual(result["status"], "gap")
        self.assertNotIn("exit_code", result)

    def test_symlink_evidence_is_rejected_before_any_file_is_published(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / "sentinel"
            sentinel.write_text("unchanged")
            output = root / "out"
            output.mkdir()
            data = io.BytesIO()
            with tarfile.open(fileobj=data, mode="w") as archive:
                regular = tarfile.TarInfo("result.json")
                regular.size = 2
                archive.addfile(regular, io.BytesIO(b"{}"))
                link = tarfile.TarInfo("runner-result.json")
                link.type = tarfile.SYMTYPE
                link.linkname = str(sentinel)
                archive.addfile(link)
            data.seek(0)
            with tarfile.open(fileobj=data, mode="r:") as archive:
                with self.assertRaises(ValueError):
                    publish_evidence(archive, output)
            self.assertEqual(sentinel.read_text(), "unchanged")
            self.assertEqual(list(output.iterdir()), [])

    def test_regular_evidence_cannot_overwrite_an_existing_host_entry(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            (output / "result.json").write_text("existing")
            data = io.BytesIO()
            with tarfile.open(fileobj=data, mode="w") as archive:
                member = tarfile.TarInfo("result.json")
                member.size = 2
                archive.addfile(member, io.BytesIO(b"{}"))
            data.seek(0)
            with tarfile.open(fileobj=data, mode="r:") as archive:
                with self.assertRaises(FileExistsError):
                    publish_evidence(archive, output)
            self.assertEqual((output / "result.json").read_text(), "existing")

    def test_unknown_assertion_is_rejected_instead_of_ignored(self):
        self.case["expect"]["stdout_contians"] = ["misspelled"]
        with self.assertRaises(ValueError):
            validate(self.artifact, self.suite)

    def test_missing_exact_revision_is_rejected(self):
        self.artifact["source"]["revision"] = "main"
        with self.assertRaises(ValueError):
            validate(self.artifact, self.suite)

    def test_duplicate_case_names_and_unbounded_timeout_rejected(self):
        duplicate = copy.deepcopy(self.case)
        self.suite["cases"].append(duplicate)
        with self.assertRaises(ValueError):
            validate(self.artifact, self.suite)
        self.suite["cases"].pop()
        self.case["timeout_seconds"] = 1000
        with self.assertRaises(ValueError):
            validate(self.artifact, self.suite)


if __name__ == "__main__":
    unittest.main()
