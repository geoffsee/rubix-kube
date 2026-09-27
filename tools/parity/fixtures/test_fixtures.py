"""Compare the frozen independent Go capture and deliberately damaged behavior."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import tarfile

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("fixture_assertions", ROOT.parent / "driver.py")
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class IndependentFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.suite = json.loads((ROOT / "config-command.json").read_text())
        cls.capture = json.loads((ROOT / "evidence/go-reference.json").read_text())

    def test_fixture_files_and_capture_bytes_match_recorded_provenance(self):
        provenance = json.loads((ROOT / "provenance.json").read_text())
        suite_digest = hashlib.sha256((ROOT / "config-command.json").read_bytes()).hexdigest()
        self.assertEqual(provenance["suite_sha256"], suite_digest)
        self.assertEqual(self.capture["result"]["suite_sha256"], suite_digest)
        self.assertEqual((ROOT / "defaults.yaml").read_text(),
                         self.capture["observations"]["default-document"]["stdout"])
        for case in self.capture["result"]["cases"]:
            observation = self.capture["observations"][case["id"]]
            for stream in ("stdout", "stderr"):
                self.assertEqual(hashlib.sha256(observation[stream].encode()).hexdigest(),
                                 case[stream + "_sha256"])
        self.assertEqual(self.capture["verified_cleanup"],
                         {"container": True, "image": True, "volume": True})

    def test_real_negative_controls_fail_with_preserved_identity_and_cleanup(self):
        controls = json.loads((ROOT / "evidence/negative-controls.json").read_text())
        mutated = (ROOT / "evidence/deliberate-mismatch-suite.json").read_bytes()
        self.assertEqual(hashlib.sha256(mutated).hexdigest(),
                         controls["deliberate_changed_expectation"]["result"]["suite_sha256"])
        for name in ("deliberate_changed_expectation", "actual_rust_placeholder"):
            record = controls[name]
            self.assertEqual(record["result"]["status"], "failed")
            self.assertTrue(all(case["status"] == "failed" for case in record["result"]["cases"]))
            self.assertEqual(record["runner"]["exit_code"], 1)
            self.assertEqual(record["runner"]["errors"], [])
            self.assertEqual(record["verified_cleanup"],
                             {"container": True, "image": True, "volume": True})
        self.assertEqual(controls["actual_rust_placeholder"]["result"]["artifact"]["kind"], "rust")

    def test_review_added_inputs_change_real_reference_behavior(self):
        controls = json.loads((ROOT / "evidence/negative-controls.json").read_text())
        negative = controls["input_sensitivity"]
        mutation_bytes = (ROOT / "evidence/input-sensitivity-suite.json").read_bytes()
        self.assertEqual(hashlib.sha256(mutation_bytes).hexdigest(), negative["result"]["suite_sha256"])
        mutated = json.loads(mutation_bytes)
        original = {c["id"]: c for c in self.suite["cases"]}
        self.assertEqual(len(mutated["cases"]), 3)
        self.assertEqual(negative["runner"]["exit_code"], 1)
        self.assertEqual(negative["runner"]["errors"], [])
        self.assertTrue(all(negative["verified_cleanup"].values()))
        for case in mutated["cases"]:
            with self.subTest(case=case["id"]):
                self.assertEqual(case["expect"], original[case["id"]]["expect"])
                self.assertNotEqual(case, original[case["id"]])
                observed = negative["observations"][case["id"]]
                self.assertEqual(observed["exit_code"], 0)
                self.assertTrue(DRIVER.assess(original[case["id"]], observed["exit_code"],
                                             observed["stdout"], observed["stderr"]))
                successful = self.capture["observations"][case["id"]]
                self.assertNotEqual(observed["stdout"], successful["stdout"])

    def test_vm_captures_cover_current_suite_and_preserve_raw_diagnostics(self):
        vm = ROOT / "evidence/vm"
        hashes = json.loads((vm / "sha256.json").read_text())
        for name, expected in hashes.items():
            self.assertEqual(hashlib.sha256((vm / name).read_bytes()).hexdigest(), expected)
        suite_hash = hashlib.sha256((ROOT / "config-command.json").read_bytes()).hexdigest()
        for kind, status in (("go", "passed"), ("rust", "failed")):
            result = json.loads((vm / kind / "result.json").read_text())
            self.assertEqual(result["suite_sha256"], suite_hash)
            self.assertEqual(result["status"], status)
            self.assertEqual({c["id"] for c in result["cases"]}, {c["id"] for c in self.suite["cases"]})
            self.assertTrue(result["owned_process_group_absent"])
            self.assertTrue(result["owned_temporary_directory_removed"])
            self.assertEqual(result["errors"], [])
            with tarfile.open(vm / kind / "diagnostics.tar.gz") as archive:
                for index, case in enumerate(result["cases"]):
                    for stream in ("stdout", "stderr"):
                        raw = archive.extractfile(f"case-{index:03d}.{stream}").read()
                        self.assertEqual(hashlib.sha256(raw).hexdigest(), case[stream + "_sha256"])

    def test_frozen_reference_observations_satisfy_every_case(self):
        for case in self.suite["cases"]:
            with self.subTest(case=case["id"]):
                observation = self.capture["observations"][case["id"]]
                self.assertEqual(DRIVER.assess(case, observation["exit_code"],
                                              observation["stdout"], observation["stderr"]), [])

    def test_changed_storage_default_fails_independent_expectation(self):
        case = self.suite["cases"][0]
        observation = self.capture["observations"][case["id"]]
        broken = observation["stdout"].replace("  localPath:\n    enabled: true\n",
                                               "  localPath:\n    enabled: false\n")
        self.assertNotEqual(broken, observation["stdout"])
        self.assertTrue(DRIVER.assess(case, observation["exit_code"], broken, observation["stderr"]))

    def test_unknown_key_changed_from_warning_to_fatal_fails(self):
        case = next(c for c in self.suite["cases"] if c["id"] == "unknown-field-is-warning")
        observation = self.capture["observations"][case["id"]]
        self.assertTrue(DRIVER.assess(case, 1, "", observation["stderr"]))

    def test_file_environment_precedence_reversal_fails(self):
        case = next(c for c in self.suite["cases"] if c["id"] == "environment-over-file")
        observation = self.capture["observations"][case["id"]]
        wrong = observation["stdout"].replace("  mtu: 1450\n", "  mtu: 1400\n")
        self.assertNotEqual(wrong, observation["stdout"])
        self.assertTrue(DRIVER.assess(case, 0, wrong, observation["stderr"]))

    def test_expected_exit_cannot_hide_a_behavior_mismatch(self):
        case = copy.deepcopy(self.suite["cases"][0])
        self.assertTrue(DRIVER.assess(case, 0, "placeholder executable\n", ""))

    def test_capture_has_exact_revision_and_complete_case_ownership(self):
        self.assertEqual(self.capture["result"]["artifact"]["source"]["revision"],
                         "2ef1c4787989f11f868f81bb84ae2afd4a49a81d")
        provenance = json.loads((ROOT / "provenance.json").read_text())
        ids = {case["id"] for case in self.suite["cases"]}
        self.assertEqual(set(provenance["cases"]), ids)
        self.assertEqual(set(self.capture["observations"]), ids)
        self.assertEqual(self.capture["runner"]["errors"], [])
        self.assertEqual(self.capture["runner"]["exit_code"], 0)


if __name__ == "__main__":
    unittest.main()
