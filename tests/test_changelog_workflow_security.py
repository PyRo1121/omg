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

    def test_generated_updates_cannot_bypass_main_review_rules(self):
        benchmark = WORKFLOW.with_name("benchmark.yml").read_text(encoding="utf-8")
        for workflow in (TEXT, benchmark):
            self.assertNotIn("contents: write", workflow)
            self.assertNotIn("git push", workflow)
            self.assertNotIn("secrets.GITHUB_TOKEN", workflow)
            self.assertIn("actions/upload-artifact@", workflow)
            self.assertIn("persist-credentials: false", workflow)
        self.assertIn("retention-days: 30", TEXT)
        self.assertIn("benchmark-history-for-review", benchmark)


if __name__ == "__main__":
    unittest.main()
