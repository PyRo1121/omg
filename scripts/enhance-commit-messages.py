#!/usr/bin/env python3
"""
OMG Commit Message Enhancer

Transforms terse/technical git commits into user-focused changelog entries.
Uses Claude AI to rewrite commit messages with context about OMG's purpose.

Usage:
    # Enhance commits from last release
    ./scripts/enhance-commit-messages.py

    # Enhance specific range
    ./scripts/enhance-commit-messages.py --from v0.1.135 --to HEAD

    # Dry run (preview without applying)
    ./scripts/enhance-commit-messages.py --dry-run

    # Interactive mode (review each change)
    ./scripts/enhance-commit-messages.py --interactive
"""

import argparse
import subprocess
import sys
import os
import json
from typing import List, Dict, Optional
from dataclasses import dataclass
from pathlib import Path

# ANSI colors
class Color:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    MAGENTA = '\033[0;35m'
    CYAN = '\033[0;36m'
    NC = '\033[0m'  # No Color
    BOLD = '\033[1m'


@dataclass
class Commit:
    hash: str
    subject: str
    body: str
    files_changed: List[str]
    stats: str


class CommitEnhancer:
    """Enhances commit messages for better changelogs"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.context = self._load_project_context()

    def _load_project_context(self) -> str:
        """Load context about OMG for AI to understand the project"""
        readme_path = self.project_root / "README.md"
        context = """
OMG is a unified package manager for Linux that replaces multiple tools:
- System packages: pacman, yay, apt
- Runtime managers: nvm, pyenv, rustup, rbenv, jenv

Key features:
- 22x faster searches than pacman (6ms vs 133ms)
- 59-483x faster than apt-cache on Debian/Ubuntu
- Unified CLI for system packages + 8 language runtimes
- Enterprise security: SLSA, PGP, SBOM, audit logs
- Team synchronization with omg.lock files

Target audience: Developers and DevOps engineers who want speed and simplicity.
"""
        if readme_path.exists():
            with open(readme_path) as f:
                context += f"\n\nREADME excerpt:\n{f.read()[:2000]}"

        return context

    def get_commits(self, from_ref: str, to_ref: str) -> List[Commit]:
        """Get commits in range"""
        cmd = [
            'git', 'log',
            f'{from_ref}..{to_ref}',
            '--format=%H|||%s|||%b|||END_COMMIT',
            '--name-only'
        ]

        result = subprocess.run(
            cmd,
            cwd=self.project_root,
            capture_output=True,
            text=True,
            check=True
        )

        commits = []
        raw_commits = result.stdout.split('|||END_COMMIT')

        for raw in raw_commits:
            if not raw.strip():
                continue

            parts = raw.split('|||')
            if len(parts) < 3:
                continue

            commit_hash = parts[0].strip()
            subject = parts[1].strip()
            body_and_files = parts[2] if len(parts) > 2 else ""

            # Split body from file list
            lines = body_and_files.split('\n')
            body_lines = []
            file_lines = []
            in_files = False

            for line in lines:
                if not in_files and (line.startswith('src/') or
                                      line.startswith('docs/') or
                                      line.startswith('tests/') or
                                      line.startswith('Cargo.') or
                                      line.startswith('scripts/')):
                    in_files = True
                    file_lines.append(line)
                elif in_files:
                    file_lines.append(line)
                else:
                    body_lines.append(line)

            body = '\n'.join(body_lines).strip()
            files = [f for f in file_lines if f.strip()]

            # Get diff stats
            stats = self._get_diff_stats(commit_hash)

            commits.append(Commit(
                hash=commit_hash,
                subject=subject,
                body=body,
                files_changed=files,
                stats=stats
            ))

        return commits

    def _get_diff_stats(self, commit_hash: str) -> str:
        """Get diff stats for commit"""
        result = subprocess.run(
            ['git', 'show', '--stat', '--format=', commit_hash],
            cwd=self.project_root,
            capture_output=True,
            text=True
        )
        return result.stdout.strip()

    def needs_enhancement(self, commit: Commit) -> bool:
        """Check if commit message needs enhancement"""
        # Skip if already detailed
        if len(commit.body) > 200:
            return False

        # Skip release commits
        if commit.subject.startswith('Release v') or \
           commit.subject.startswith('Bump version'):
            return False

        # Enhance if too terse
        terse_patterns = [
            'fix', 'update', 'add', 'remove', 'chore',
            'refactor', 'notes', 'wip', 'tmp'
        ]

        subject_lower = commit.subject.lower()
        if any(subject_lower == pattern or
               subject_lower.startswith(pattern + ':') or
               subject_lower.startswith(pattern + '(')
               for pattern in terse_patterns):
            # But only if very short
            if len(commit.subject) < 60:
                return True

        return False

    def enhance_commit(self, commit: Commit) -> Optional[str]:
        """Generate enhanced commit message"""
        # This would ideally call Claude API, but for now we'll provide a template
        # that users can customize with their AI workflow

        enhanced_template = f"""Original: {commit.subject}

Files changed:
{chr(10).join(f'  - {f}' for f in commit.files_changed[:10])}

Diff stats:
{commit.stats[:500]}

[AI Enhancement Instructions]
Rewrite this commit as a user-focused changelog entry:

1. Focus on USER IMPACT, not implementation details
2. Explain WHAT changed (for users) and WHY it matters
3. Use clear, jargon-free language
4. Format: "<type>(<scope>): <clear description>"
5. Types: feat, fix, perf, docs, refactor, test, chore
6. Add a detailed body if needed explaining the benefit

Context about OMG:
{self.context[:500]}

Example good messages:
- "feat(debian): incremental index updates for 3-5x faster package operations"
- "fix(cli): ensure sudo prompts work correctly in interactive mode"
- "perf(search): switch to LZ4 compression for 60% smaller cache and faster I/O"

Generate enhanced commit message:
"""
        return enhanced_template

    def preview_enhancements(self, commits: List[Commit], limit: int = 10):
        """Preview commits that need enhancement"""
        print(f"\n{Color.BOLD}Commits that could be enhanced:{Color.NC}\n")

        enhanced_count = 0
        for commit in commits[:limit]:
            if self.needs_enhancement(commit):
                enhanced_count += 1
                print(f"{Color.YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{Color.NC}")
                print(f"{Color.CYAN}Hash:{Color.NC} {commit.hash[:8]}")
                print(f"{Color.CYAN}Current:{Color.NC} {commit.subject}")
                print(f"{Color.CYAN}Files:{Color.NC} {len(commit.files_changed)} changed")
                print(f"\n{Color.MAGENTA}Enhancement template:{Color.NC}")
                template = self.enhance_commit(commit)
                print(template[:500] + "..." if len(template) > 500 else template)
                print()

        print(f"{Color.YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{Color.NC}")
        print(f"\n{Color.GREEN}✓{Color.NC} Found {enhanced_count} commits that could be enhanced")

        if enhanced_count > 0:
            print(f"\n{Color.BLUE}Tip:{Color.NC} To apply enhancements:")
            print(f"  1. Use the enhancement templates above with Claude/ChatGPT")
            print(f"  2. Apply via: git rebase -i {commits[-1].hash}~1")
            print(f"  3. Mark commits as 'reword' and paste enhanced messages")


def main():
    parser = argparse.ArgumentParser(
        description='Enhance commit messages for better changelogs',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Preview commits that need enhancement
  %(prog)s

  # Specify commit range
  %(prog)s --from v0.1.135 --to HEAD

  # Show more commits
  %(prog)s --limit 20

This tool identifies terse commits and provides templates for enhancement.
Use the templates with AI (Claude/ChatGPT) to generate user-focused messages.
        """
    )

    parser.add_argument(
        '--from',
        dest='from_ref',
        default=None,
        help='Starting commit/tag (default: last tag)'
    )

    parser.add_argument(
        '--to',
        dest='to_ref',
        default='HEAD',
        help='Ending commit/tag (default: HEAD)'
    )

    parser.add_argument(
        '--limit',
        type=int,
        default=10,
        help='Maximum commits to preview (default: 10)'
    )

    args = parser.parse_args()

    # Find project root
    script_dir = Path(__file__).parent
    project_root = script_dir.parent

    # Determine from_ref if not specified
    from_ref = args.from_ref
    if from_ref is None:
        # Get last tag
        result = subprocess.run(
            ['git', 'describe', '--tags', '--abbrev=0'],
            cwd=project_root,
            capture_output=True,
            text=True
        )
        if result.returncode == 0:
            from_ref = result.stdout.strip()
        else:
            print(f"{Color.RED}✗{Color.NC} Could not find last tag. Specify --from manually.")
            sys.exit(1)

    print(f"{Color.BLUE}ℹ{Color.NC} Analyzing commits from {from_ref} to {args.to_ref}")

    enhancer = CommitEnhancer(project_root)

    try:
        commits = enhancer.get_commits(from_ref, args.to_ref)
        print(f"{Color.GREEN}✓{Color.NC} Found {len(commits)} commits")

        enhancer.preview_enhancements(commits, limit=args.limit)

    except subprocess.CalledProcessError as e:
        print(f"{Color.RED}✗{Color.NC} Git command failed: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"{Color.RED}✗{Color.NC} Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()
