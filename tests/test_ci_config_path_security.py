"""Security invariants for generated CI configuration paths."""

from pathlib import Path
import unittest


SOURCE = (Path(__file__).parents[1] / "src" / "cli" / "ci.rs").read_text(
    encoding="utf-8"
)


class CiConfigPathSecurityTests(unittest.TestCase):
    def test_generation_atomically_refuses_existing_entries(self):
        writer = SOURCE.split("fn write_config_file", 1)[1].split(
            "fn generate_github_actions", 1
        )[0]
        self.assertIn("create_new(true)", writer)
        self.assertNotIn("fs::write(path, config)", writer)


if __name__ == "__main__":
    unittest.main()
