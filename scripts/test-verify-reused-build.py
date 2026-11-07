#!/usr/bin/env python3
"""Release reuse must reject changed application inputs or incomplete evidence."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('reuse', Path(__file__).with_name('verify-reused-build.py'))
reuse = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reuse)

class ReuseGate(unittest.TestCase):
    def setUp(self):
        self.run = {'name':'Build desktop packages', 'head_repository':{'full_name':'JOJO-5/daxiong-pet'}, 'head_sha':'a'*40}
        self.jobs = [{'name':name,'status':'completed','conclusion':'success'} for name in reuse.PLATFORMS]
        self.artifacts = [{'name':name,'expired':False,'size_in_bytes':100,
                           'workflow_run':{'head_sha':'a'*40}} for name in reuse.ARTIFACTS]
    def check(self, changed=(), **overrides):
        reuse.validate(overrides.get('run',self.run), overrides.get('jobs',self.jobs),
                       overrides.get('artifacts',self.artifacts), changed, 'JOJO-5/daxiong-pet')
    def test_only_infrastructure_or_documentation_can_change(self):
        self.check(['scripts/e2e-chat-motion.py','.github/workflows/build-desktop.yml','.github/workflows/e2e-desktop.yml','docs/validation/result.json'])
    def test_production_and_resource_inputs_cannot_change(self):
        for name in ['src/App.tsx','src-tauri/src/engine.rs','public/spritesheet-extended.webp',
                     'package.json','src-tauri/Cargo.lock','scripts/ai-prototype/default-model.json',
                     'native/ocr/main.cpp','scripts/build-macos-dmg.sh']:
            with self.subTest(name=name), self.assertRaises(AssertionError): self.check([name])
    def test_failed_or_missing_platform_cannot_reuse(self):
        for jobs in [self.jobs[:-1], [dict(j,conclusion='failure') for j in self.jobs]]:
            with self.assertRaises(AssertionError): self.check(jobs=jobs)
    def test_missing_or_expired_artifacts_cannot_reuse(self):
        with self.assertRaises(AssertionError): self.check(artifacts=self.artifacts[:-1])
        artifacts=copy.deepcopy(self.artifacts);artifacts[0]['expired']=True
        with self.assertRaises(AssertionError): self.check(artifacts=artifacts)
    def test_wrong_artifact_source_cannot_reuse(self):
        artifacts=copy.deepcopy(self.artifacts);artifacts[0]['workflow_run']['head_sha']='b'*40
        with self.assertRaises(AssertionError): self.check(artifacts=artifacts)
    def test_foreign_repository_cannot_reuse(self):
        run=copy.deepcopy(self.run);run['head_repository']['full_name']='other/repo'
        with self.assertRaises(AssertionError): self.check(run=run)

if __name__ == '__main__': unittest.main()
