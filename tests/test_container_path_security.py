"""Security invariants for generated container configuration paths."""

from pathlib import Path
import unittest


SOURCE = (Path(__file__).parents[1] / "src" / "cli" / "container.rs").read_text(
    encoding="utf-8"
)


class ContainerPathSecurityTests(unittest.TestCase):
    def test_generation_atomically_refuses_existing_entries(self):
        initializer = SOURCE.split("pub fn init(base_image", 1)[1].split(
            "pub fn build", 1
        )[0]
        self.assertIn("create_new(true)", initializer)
        self.assertNotIn("std::fs::write(&dockerfile_path", initializer)


if __name__ == "__main__":
    unittest.main()
