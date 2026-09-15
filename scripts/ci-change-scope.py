#!/usr/bin/env python3
"""Conservatively identify documentation-only PRs without GitHub API pagination."""
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess


def documentation_only(paths):
    return bool(paths) and all(
        path in ('README.md', 'CHANGELOG.md') or
        (path.startswith('docs/') and path.endswith('.md') and
         '..' not in PurePosixPath(path).parts)
        for path in paths
    )


def requires_build(event_name, event, cwd=None):
    if event_name != 'pull_request':
        return True
    base = event['pull_request']['base']['sha']
    head = event['pull_request']['head']['sha']
    if not all(isinstance(sha, str) and re.fullmatch(r'[0-9a-f]{40}', sha)
               for sha in (base, head)):
        raise ValueError('Invalid PR commit identity')
    # Disable rename folding: moving code into docs must include the deleted
    # source path. NUL separators preserve whitespace and unusual filenames.
    result = subprocess.run(
        ['git', 'diff', '--name-only', '--no-renames', '-z', f'{base}...{head}', '--'],
        cwd=cwd, check=True, capture_output=True, timeout=60,
    )
    paths = result.stdout.decode('utf-8', errors='surrogateescape').split('\0')
    return not documentation_only([path for path in paths if path])


if __name__ == '__main__':
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    required = requires_build(os.environ['GITHUB_EVENT_NAME'], event)
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
        output.write(f'required={str(required).lower()}\n')
    print('Full checks required' if required else 'Documentation-only PR: compilation not required')
