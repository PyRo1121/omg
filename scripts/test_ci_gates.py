from __future__ import annotations

import re
import unittest
from pathlib import Path

CI_YML = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "ci.yml"


def job_block(text: str, job: str) -> str:
    """Return the YAML block for `job:` up to the next top-level job key."""
    start = text.index(f"\n  {job}:")
    rest = text[start + 1 :]
    nxt = re.search(r"\n  [a-zA-Z0-9_-]+:\n", rest[1:])
    end = start + 1 + nxt.start() + 1 if nxt else len(text)
    return text[start:end]


class QuickGateOfflineTests(unittest.TestCase):
    def test_quick_gate_does_no_network_install_and_keeps_gate_fast(self) -> None:
        text = CI_YML.read_text(encoding="utf-8")
        gate = job_block(text, "quick-gate")
        self.assertNotIn(
            "apt-get",
            gate,
            "quick-gate must not apt-install (network); "
            "shell-completion check belongs in the portable job",
        )
        self.assertNotIn(
            "shell_completion.zsh",
            gate,
            "quick-gate must not run the interactive zsh completion test",
        )
        m = re.search(r"timeout-minutes:\s*(\d+)", gate)
        self.assertIsNotNone(m, "quick-gate must declare timeout-minutes")
        self.assertLessEqual(
            int(m.group(1)),
            10,
            f"quick-gate timeout must stay <= 10 min: the gate really "
            f"executes make ci-local-quick (~4.5 min cold), got {m.group(1)}",
        )
        self.assertIn(
            "Swatinem/rust-cache",
            gate,
            "quick-gate must restore the Rust cache or every run pays a "
            "cold 4+ minute check and the gate is not quick at all",
        )
        portable = job_block(text, "portable")
        self.assertIn(
            "shell_completion.zsh",
            portable,
            "portable job must own the relocated shell-completion check",
        )
        self.assertIn(
            "--error-on=any",
            portable,
            "portable apt update must fail closed with --error-on=any + retry",
        )
        self.assertIn(
            "timeout -k 10s 30s zsh tests/shell_completion.zsh",
            portable,
            "relocated completion check must keep its timeout bound",
        )


class LocalCiGateExecutesTests(unittest.TestCase):
    def test_local_ci_gate_executes_recipes_instead_of_dry_run(self) -> None:
        text = CI_YML.read_text(encoding="utf-8")
        gate = job_block(text, "quick-gate")
        self.assertNotIn(
            "make -n",
            gate,
            "quick-gate must not use `make -n` dry-run: it never executes recipes",
        )
        self.assertRegex(
            gate,
            r"run:\s*make ci-local-quick",
            "quick-gate must really execute `make ci-local-quick`",
        )


if __name__ == "__main__":
    unittest.main()
