"""Run workflow selection and failure gates locally with Bash and jq (no guests)."""
import itertools
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import textwrap
import unittest

WORKFLOW = Path(__file__).resolve().parents[1] / '.github/workflows/qemu-matrix.yml'
TEXT = WORKFLOW.read_text(encoding='utf-8')


def literal(text, key, indent):
    match = re.search(r'^' + ' ' * indent + re.escape(key) + r': \|\n((?: {' + str(indent + 1) + r',}[^\n]*\n|\n)*)', text, re.M)
    if match is None:
        raise AssertionError(f'Missing literal {key}')
    return textwrap.dedent(match[1])


def step(name):
    start = TEXT.index('      - name: ' + name + '\n')
    remaining = TEXT[start + 1:]
    end = re.search(r'^      - |^  [a-z][\w-]*:', remaining, re.M)
    return TEXT[start:start + 1 + end.start()] if end else TEXT[start:]


class QemuWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.bash = os.environ.get('OMG_TEST_BASH') or shutil.which('bash')
        if not cls.bash or not shutil.which('jq'):
            raise RuntimeError('Workflow regression tests require Bash and jq on PATH')

    def run_script(self, script, env, directory):
        environment = dict(os.environ, **env)
        environment['GITHUB_OUTPUT'] = (directory / 'output').as_posix()
        environment['GITHUB_STEP_SUMMARY'] = (directory / 'summary').as_posix()
        environment['GITHUB_SHA'] = 'f' * 40
        environment['GITHUB_REPOSITORY'] = 'test/omg'
        result = subprocess.run([self.bash, '--noprofile', '--norc', '-euo', 'pipefail', '-c', script],
                                cwd=WORKFLOW.parents[2], env=environment, text=True, capture_output=True)
        output = directory / 'output'
        values = dict(line.split('=', 1) for line in output.read_text().splitlines()) if output.exists() else {}
        return result, values

    def selection(self, directory, staged, distro, arch, tag=''):
        block = step('Resolve selection')
        env = dict(STAGED=str(staged).lower(), REQUESTED_DISTRO=distro,
                   REQUESTED_ARCH=arch, REQUESTED_RELEASE_TAG=tag,
                   BUILD_X64=literal(block, 'BUILD_X64', 10), BUILD_ARM64=literal(block, 'BUILD_ARM64', 10))
        # A local shell function replaces the only release API call.
        return self.run_script('gh() { printf "v9.8.7\\n"; }\n' + literal(block, 'run', 8), env, directory)

    def test_dispatch_cross_product(self):
        for staged, distro, arch in itertools.product([False, True], ['all', 'arch', 'debian', 'ubuntu', 'fedora'], ['all', 'x64', 'arm64']):
            with self.subTest(staged=staged, distro=distro, arch=arch), tempfile.TemporaryDirectory() as tmp:
                result, values = self.selection(Path(tmp), staged, distro, arch)
                invalid = arch == 'arm64' and (not staged or distro == 'arch')
                self.assertEqual(result.returncode != 0, invalid, result.stderr)
                if invalid:
                    continue
                self.assertEqual(values['x64'], str(arch != 'arm64').lower())
                self.assertEqual(values['arm64'], str(staged and arch != 'x64' and distro != 'arch').lower())
                self.assertEqual(json.loads(values['distros']), ['arch', 'debian', 'ubuntu', 'fedora'] if distro == 'all' else [distro])
                for key, allowed in [('build-x64', ['arch', 'debian', 'fedora']), ('build-arm64', ['debian', 'fedora'])]:
                    self.assertEqual([row['distro'] for row in json.loads(values[key])], allowed if distro == 'all' else [distro] if distro in allowed else [])
                self.assertEqual(values['tag'], 'v' + re.search(r'^version = "([^"]+)"', (WORKFLOW.parents[2] / 'Cargo.toml').read_text(), re.M)[1] if staged else 'v9.8.7')

    def test_staged_tag_conflict_and_bad_release_fail(self):
        for staged, tag in [(True, 'v1.2.3'), (False, 'invalid'), (False, 'v1.2.3\nother')]:
            with self.subTest(staged=staged, tag=tag), tempfile.TemporaryDirectory() as tmp:
                result, _ = self.selection(Path(tmp), staged, 'all', 'all', tag)
                self.assertNotEqual(result.returncode, 0)

    def test_explicit_release_is_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, values = self.selection(Path(tmp), False, 'debian', 'x64', 'v1.2.3')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(values['tag'], 'v1.2.3')

    def test_summary_rejects_failed_cancelled_and_missing_selected_jobs(self):
        script = literal(step('Summarize and require selected guest jobs'), 'run', 8)
        for x64, arm in itertools.product([False, True], repeat=2):
            good = {'prepare': {'result': 'success', 'outputs': {'x64': str(x64).lower(), 'arm64': str(arm).lower()}},
                    'build-staged': {'result': 'skipped'},
                    'guest': {'result': 'success' if x64 else 'skipped'},
                    'guest-arm': {'result': 'success' if arm else 'skipped'}}
            variants = [(good, True)]
            for job in good:
                for status in ['failure', 'cancelled'] + (['skipped'] if job == 'prepare' or job == 'guest' and x64 or job == 'guest-arm' and arm else []):
                    changed = json.loads(json.dumps(good))
                    changed[job]['result'] = status
                    variants.append((changed, False))
            for values, expected in variants:
                with self.subTest(values=values), tempfile.TemporaryDirectory() as tmp:
                    result, _ = self.run_script(script, {'JOB_RESULTS': json.dumps(values)}, Path(tmp))
                    self.assertEqual(result.returncode == 0, expected, result.stderr)


class ReportingIntegrationTests(unittest.TestCase):
    def test_guests_configure_reporter_and_preserve_delivery_logs(self):
        for job in ('guest', 'guest-arm'):
            body = TEXT.split('\n  ' + job + ':\n', 1)[1].split('\n  #', 1)[0]
            self.assertIn('run: python3 scripts/ci-smoke-report.py configure', body)
            self.assertIn('OMG_SMOKE_SENTRY_DSN: ${{ secrets.OMG_SMOKE_SENTRY_DSN }}', body)
            self.assertIn('find "$RUNNER_TEMP/qemu-evidence" -name reporting.log', body)
            self.assertNotIn('${{ env.HOME }}', body)
            # Upload only diagnostic file types; failed cleanup may retain
            # private keys, cloud-init configuration and guest disk images.
            paths = re.findall(r'^            (\$\{\{ runner.temp \}\}/qemu-evidence/.*)$', body, re.M)
            self.assertTrue(paths)
            self.assertTrue(all(path.endswith(('.json', '.jsonl', '.log', '.txt', '.tsv', '.md', '.csv')) for path in paths))

    def test_workflow_failure_reports_even_when_guests_never_start(self):
        body = TEXT.split('\n  summary:\n', 1)[1]
        self.assertIn('if: always()', body)
        self.assertIn('build-staged', body)
        self.assertIn('--case-id qemu-matrix-workflow --status failure', body)
        self.assertIn('if: always() && failure()', body)
        self.assertIn('OMG_SMOKE_ENVIRONMENT: qemu-matrix', body)


if __name__ == '__main__':
    unittest.main()
