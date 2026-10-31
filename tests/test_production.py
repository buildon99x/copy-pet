"""Real local Git remotes; only GitHub's external release response is substituted."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('production', Path(__file__).parents[1] / 'scripts/production.py')

class ProductionTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict(os.environ, {"GITHUB_REF_TYPE":"branch"})
        environment.start()
        self.addCleanup(environment.stop)
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.remote = self.root / 'remote.git'
        self.repo = self.root / 'repo'
        subprocess.run(['git', 'init', '--bare', '-q', str(self.remote)], check=True)
        self.repo.mkdir()
        self.git('init', '-q', '-b', 'main')
        self.git('config', 'user.email', 'test@example.com')
        self.git('config', 'user.name', 'Test')
        self.git('config', 'commit.gpgsign', 'false')
        self.git('config', 'tag.gpgsign', 'false')
        self.git('remote', 'add', 'origin', str(self.remote))
        self.old = self.commit('1.0.0')
        self.git('tag', 'v1.0.0')
        self.git('branch', 'production')
        self.git('push', '-q', 'origin', 'main', 'production', '--tags')
        self.new = self.commit('1.1.0')
        self.git('tag', 'v1.1.0')
        self.git('push', '-q', 'origin', 'main', '--tags')
        self.module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(self.module)
        self.module.ROOT = self.repo
        self.published = {'tagName':'v1.1.0','isDraft':False,'isPrerelease':False,
                          'assets':[{'name':n} for n in self.module.ASSETS]}

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.repo, stderr=subprocess.DEVNULL, text=True).strip()

    def commit(self, version):
        (self.repo/'Cargo.toml').write_text('[package]\nname = "clipcat"\nversion = "'+version+'"\n')
        self.git('add', '.'); self.git('commit', '-qm', version)
        return self.git('rev-parse', 'HEAD')

    def production(self):
        return self.git('ls-remote', 'origin', 'refs/heads/production').split()[0]

    def test_check_does_not_advance_production(self):
        self.module.check('v1.1.0')
        self.assertEqual(self.production(), self.old)

    def test_published_release_advances_exact_commit_and_rerun_is_idempotent(self):
        with patch.object(self.module, 'latest_release', return_value=self.published):
            self.module.promote('v1.1.0')
            self.module.promote('v1.1.0')
            self.module.audit()
        self.assertEqual(self.production(), self.new)

    def test_draft_prerelease_wrong_tag_or_missing_assets_do_not_advance(self):
        for changed in [{'isDraft':True}, {'isPrerelease':True}, {'tagName':'v1.0.0'}, {'assets':[]}]:
            with self.subTest(changed=changed), patch.object(self.module, 'latest_release', return_value={**self.published, **changed}):
                with self.assertRaises(RuntimeError):self.module.promote('v1.1.0')
                self.assertEqual(self.production(), self.old)

    def test_feature_only_tag_is_rejected(self):
        self.git('checkout', '-qb', 'feature')
        self.commit('1.2.0'); self.git('tag', 'v1.2.0')
        with self.assertRaises(RuntimeError):self.module.check('v1.2.0')

    def test_version_mismatch_is_rejected(self):
        self.git('tag', 'v9.0.0')
        with self.assertRaises(RuntimeError):self.module.check('v9.0.0')

    def test_old_release_cannot_roll_back_production(self):
        with patch.object(self.module, 'latest_release', return_value=self.published):self.module.promote('v1.1.0')
        with self.assertRaises(RuntimeError):self.module.check('v1.0.0')
        self.assertEqual(self.production(), self.new)

    def test_tag_moved_after_workflow_started_is_rejected(self):
        with patch.dict(os.environ, {'GITHUB_REF_TYPE':'tag', 'GITHUB_SHA':self.old}):
            with self.assertRaises(RuntimeError):self.module.check('v1.1.0')
        self.assertEqual(self.production(), self.old)

    def test_annotated_workflow_sha_resolves_to_the_build_commit(self):
        self.git('tag', '-a', 'v1.1.0-annotated', '-m', 'annotated workflow event', self.new)
        with patch.dict(os.environ, {'GITHUB_REF_TYPE':'tag', 'GITHUB_SHA':self.git('rev-parse','v1.1.0-annotated')}):
            self.module.check('v1.1.0')

    def test_refresh_handles_checkout_lightweight_ref_for_remote_annotated_tag(self):
        self.git('tag','-f','-a','v1.1.0','-m','release annotation',self.new)
        self.git('push','-q','--force','origin','refs/tags/v1.1.0')
        # actions/checkout may materialize a tag as the event commit locally.
        self.git('tag','-f','v1.1.0',self.new)
        self.git('checkout','--detach',self.new)
        self.module.check('v1.1.0')

    def test_concurrent_ref_change_is_not_overwritten(self):
        def competing_promotion(tag):
            self.git('push', '-q', 'origin', self.new+':refs/heads/production')
        with patch.object(self.module, 'published', side_effect=competing_promotion):
            with self.assertRaises(RuntimeError):self.module.promote('v1.1.0')
        self.assertEqual(self.production(), self.new)

    def test_ready_accepts_current_production_and_same_tag_recovery(self):
        for tag in ['v1.0.0','v1.1.0']:
            with patch.object(self.module, 'latest_release', return_value={**self.published,'tagName':tag}):
                self.module.ready('v1.1.0')
        self.assertEqual(self.production(), self.old)

    def test_ready_blocks_another_release_until_previous_publication_is_recovered(self):
        self.commit('1.2.0'); self.git('tag','v1.2.0')
        self.git('push','-q','origin','main','--tags')
        with patch.object(self.module, 'latest_release', return_value={**self.published,'tagName':'v1.2.0'}):
            with self.assertRaises(RuntimeError):self.module.ready('v1.1.0')
        self.assertEqual(self.production(),self.old)

    def test_audit_detects_published_branch_mismatch(self):
        with patch.object(self.module, 'latest_release', return_value=self.published):
            with self.assertRaises(RuntimeError):self.module.audit()

if __name__ == '__main__':unittest.main()
