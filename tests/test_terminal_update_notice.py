"""Execute the Bash hook to verify once-per-interactive-shell notice dispatch."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BASH = 'C:/Program Files/Git/bin/bash.exe' if os.name == 'nt' else 'bash'


class TerminalNoticeTests(unittest.TestCase):
    def test_notice_once_on_interactive_startup_never_on_prompt_or_script(self):
        source = (ROOT / 'src/hooks/mod.rs').read_text(encoding='utf-8')
        hook = source.split('const BASH_HOOK: &str = r#"', 1)[1].split('"#;', 1)[0]
        for interactive in (False, True):
            with self.subTest(interactive=interactive), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                command = root / 'omg'
                command.write_text('#!/bin/sh\nif [ "$1" = __update-notice ]; then echo notice >> calls; fi\n')
                command.chmod(0o700)
                (root / 'hook').write_text(hook, encoding='utf-8', newline='\n')
                args = [BASH, '--noprofile', '--norc'] + (['-i'] if interactive else [])
                script = 'export PATH="$PWD:$PATH"; source ./hook; source ./hook; _omg_hook; _omg_hook'
                result = subprocess.run(args + ['-c', script], cwd=root, capture_output=True, text=True, timeout=15)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = (root / 'calls').read_text().splitlines() if (root / 'calls').exists() else []
                self.assertEqual(calls, ['notice'] if interactive else [])


if __name__ == '__main__':
    unittest.main()
