import base64
import importlib.util
import json
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('prepare_release', Path(__file__).with_name('prepare-qemu-release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseContractTests(unittest.TestCase):
    def test_contract_is_pinned_to_resolved_release_not_main(self):
        data = (release.HEADER + '\nold-case\t[]\tread\t0\tpass\t-\thermetic\t-\t-\t-\n').encode()
        revision = 'a' * 40
        blob = {'encoding': 'base64', 'size': len(data), 'content': base64.b64encode(data).decode()}
        with tempfile.TemporaryDirectory() as tmp, patch.object(release, 'api', side_effect=[{'sha': revision}, blob, {'sha': revision}]) as api, patch.object(release.subprocess, 'run') as download:
            destination = Path(tmp)
            release.prepare('v0.1.220', 'arch', destination)
            self.assertEqual(api.call_args_list[1].args[0], f'contents/tests/cli_behavior_inventory.tsv?ref={revision}')
            self.assertEqual((destination / 'cases.tsv').read_bytes(), data)
            self.assertEqual(json.loads((destination / 'inventory-provenance.json').read_text())['inventory_revision'], revision)
            self.assertIn('omg-v0.1.220-x86_64-linux-arch.tar.gz', download.call_args_list[0].args[0])
            verify = download.call_args_list[1]
            self.assertEqual(verify.args[0], [
                'gh', 'attestation', 'verify',
                str(destination / 'omg-v0.1.220-x86_64-linux-arch.tar.gz'),
                '--repo', release.REPOSITORY,
                '--source-digest', revision,
                '--source-ref', 'refs/tags/v0.1.220',
                '--signer-workflow', release.REPOSITORY + '/.github/workflows/release.yml',
            ])
            self.assertTrue(verify.kwargs['check'])
            self.assertLessEqual(verify.kwargs['timeout'], 180)

    def test_invalid_attestation_does_not_publish_a_usable_contract(self):
        data = (release.HEADER + '\n').encode()
        blob = {'encoding': 'base64', 'size': len(data), 'content': base64.b64encode(data).decode()}
        with tempfile.TemporaryDirectory() as tmp, patch.object(release, 'api', side_effect=[{'sha': 'a' * 40}, blob, {'sha': 'a' * 40}]), patch.object(release.subprocess, 'run', side_effect=[None, subprocess.CalledProcessError(1, 'verify')]):
            with self.assertRaises(subprocess.CalledProcessError):
                release.prepare('v0.1.220', 'arch', Path(tmp))
            self.assertFalse((Path(tmp) / 'cases.tsv').exists())
            self.assertFalse((Path(tmp) / 'inventory-provenance.json').exists())

    def test_bad_inventory_cannot_fall_back_to_current_contract(self):
        for blob in ({'encoding': 'none', 'size': 2}, {'encoding': 'base64', 'size': 1048577},
                     {'encoding': 'base64', 'size': 3, 'content': 'YmFk'}):
            with self.subTest(blob=blob), tempfile.TemporaryDirectory() as tmp, patch.object(release, 'api', side_effect=[{'sha': 'a' * 40}, blob]), patch.object(release.subprocess, 'run') as download:
                with self.assertRaises(ValueError):
                    release.prepare('v0.1.220', 'debian', Path(tmp))
                download.assert_not_called()
                self.assertFalse((Path(tmp) / 'cases.tsv').exists())

    def test_moving_tag_does_not_publish_contract(self):
        data = (release.HEADER + '\n').encode()
        blob = {'encoding': 'base64', 'size': len(data), 'content': base64.b64encode(data).decode()}
        with tempfile.TemporaryDirectory() as tmp, patch.object(release, 'api', side_effect=[{'sha': 'a' * 40}, blob, {'sha': 'b' * 40}]), patch.object(release.subprocess, 'run'):
            with self.assertRaisesRegex(ValueError, 'moved'):
                release.prepare('v0.1.220', 'ubuntu', Path(tmp))
            self.assertFalse((Path(tmp) / 'cases.tsv').exists())
