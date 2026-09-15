import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("storage_faults", ROOT / "scripts/qemu-storage-faults.py")
FAULTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FAULTS)


class StorageFaultTests(unittest.TestCase):
    def setUp(self):
        self.receipt = dict(schema_version=1, scope="privacy-export-atomic-write", complete=True, cases=[
            dict(id=identity, fault_observed=True, prior_preserved=True, recovered=True)
            for identity in ("enospc", "readonly", "fsync-eio", "fsync-kill")])

    def test_complete_fault_proof_is_admitted(self):
        FAULTS.validate_receipt(self.receipt)

    def test_missing_duplicate_or_unproven_fault_fails(self):
        for field in ("fault_observed", "prior_preserved", "recovered"):
            value = copy.deepcopy(self.receipt)
            value["cases"][0][field] = False
            with self.assertRaises(ValueError):
                FAULTS.validate_receipt(value)
        for rows in (self.receipt["cases"][:3], [self.receipt["cases"][0]] * 4):
            with self.assertRaises(ValueError):
                FAULTS.validate_receipt(dict(self.receipt, cases=rows))


if __name__ == "__main__":
    unittest.main()
