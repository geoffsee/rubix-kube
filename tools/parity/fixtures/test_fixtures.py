"""Compare the frozen independent Go capture and deliberately damaged behavior."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("fixture_assertions", ROOT.parent / "driver.py")
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class IndependentFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.suite = json.loads((ROOT / "config-command.json").read_text())
        cls.capture = json.loads((ROOT / "evidence/go-reference.json").read_text())

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
