"""Regressions for maintenance jobs that are not covered by product unit tests."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class MaintenanceWorkflowTests(unittest.TestCase):
    def test_local_changelog_does_not_require_remote_api(self):
        config = tomllib.loads((ROOT / 'cliff.toml').read_text(encoding='utf-8'))
        self.assertTrue(config['remote'].get('offline', False))
        template = config['changelog']['body'] + config['changelog']['footer']
        self.assertNotRegex(template, r'\{[{%][^}]*\b(?:github|remote)\.')
        self.assertIn('https://github.com/PyRo1121/omg/compare/', template)

    def test_fuzz_invocations_select_installed_nightly_and_host_target(self):
        source = (ROOT / '.github/workflows/fuzz.yml').read_text()
        nightly = re.search(r'toolchain: (nightly-[\d-]+)', source)[1]
        self.assertIn(f'cargo +{nightly} fetch --manifest-path fuzz/Cargo.toml --locked', source)
        self.assertIn(f'cargo +{nightly} fuzz run --target x86_64-unknown-linux-gnu', source)
        self.assertIn('-max_total_time=180', source)
        self.assertNotRegex(source, r'(?m)^\s+cargo fuzz run')


if __name__ == '__main__':
    unittest.main()
