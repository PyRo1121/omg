"""Security invariants for the changelog writer workflow."""

from pathlib import Path
import unittest


WORKFLOW = Path(__file__).parents[1] / ".github" / "workflows" / "changelog.yml"
TEXT = WORKFLOW.read_text(encoding="utf-8")


class ChangelogWorkflowSecurityTests(unittest.TestCase):
    def test_generation_cannot_write_repository_contents(self):
        self.assertIn("permissions:\n  contents: read", TEXT)
        generation = TEXT.split("  generate-changelog:\n", 1)[1].split(
            "  publish-changelog:\n", 1
        )[0]
        self.assertNotIn("contents: write", generation)
        self.assertIn("taiki-e/install-action@", generation)
        self.assertIn("actions/upload-artifact@", generation)

    def test_only_publisher_receives_write_permission(self):
        publisher = TEXT.split("  publish-changelog:\n", 1)[1]
        self.assertEqual(1, TEXT.count("contents: write"))
        self.assertIn("needs: generate-changelog", publisher)
        self.assertIn("if: needs.generate-changelog.outputs.changed == 'true'", publisher)
        self.assertIn("permissions:\n      contents: write", publisher)
        self.assertIn("actions/download-artifact@", publisher)
        self.assertNotIn("taiki-e/install-action@", publisher)


if __name__ == "__main__":
    unittest.main()
