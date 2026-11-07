#!/usr/bin/env python3
"""Reuse installers only when all four platform jobs passed and app files match."""
import json
import os
from pathlib import Path
import re
import subprocess

PLATFORMS = {'Windows x64', 'macOS Apple Silicon', 'macOS Intel', 'Linux x64 (X11)'}
ARTIFACTS = {prefix + platform for prefix in ['', 'light-'] for platform in
             ['daxiong-windows-x64', 'daxiong-macos-arm64', 'daxiong-macos-x64', 'daxiong-linux-x64']}
# These files do not enter the application or its AI/resource build. Keep narrow.
INFRA_FILES = {'.github/workflows/build-desktop.yml', '.github/workflows/e2e-desktop.yml',
               'scripts/e2e-chat-motion.py',
               'scripts/verify-reused-build.py', 'scripts/test-verify-reused-build.py'}

def validate(run, jobs, artifacts, changed, repository):
    assert run['name'] == 'Build desktop packages'
    assert run['head_repository']['full_name'] == repository, 'foreign build repository'
    assert re.fullmatch(r'[0-9a-f]{40}', run['head_sha'])
    passed = {job['name'] for job in jobs if job['status'] == 'completed' and job['conclusion'] == 'success'}
    assert PLATFORMS <= passed, 'four successful platform builds required'
    recorded = {artifact['name']: artifact for artifact in artifacts}
    assert ARTIFACTS <= recorded.keys(), 'complete full/light artifacts required'
    for name in ARTIFACTS:
        artifact = recorded[name]
        assert not artifact['expired'] and artifact['size_in_bytes'] > 0, name
        assert artifact['workflow_run']['head_sha'] == run['head_sha'], 'artifact source mismatch'
    rejected = [name for name in changed if name not in INFRA_FILES and not name.startswith('docs/')]
    assert not rejected, f'application/build input changes cannot reuse installers: {rejected}'

def main():
    repository = os.environ['GITHUB_REPOSITORY']
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())
    match = re.match(r'^ci: reuse build ([0-9]+)(?:\s|$)', event['head_commit']['message'])
    assert match, 'explicit reuse commit required'
    run_id = match[1]
    def api(path):
        return json.loads(subprocess.check_output(['gh', 'api', f'repos/{repository}/{path}'], text=True))
    run = api(f'actions/runs/{run_id}')
    jobs = api(f'actions/runs/{run_id}/jobs?filter=latest&per_page=100')['jobs']
    artifacts = api(f'actions/runs/{run_id}/artifacts?per_page=100')['artifacts']
    current = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    assert current == os.environ['GITHUB_SHA'], 'checkout source mismatch'
    subprocess.run(['git', 'fetch', 'origin', run['head_sha']], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    changed = subprocess.check_output(['git', 'diff', '--no-renames', '--name-only',
                                       run['head_sha'], current], text=True).splitlines()
    validate(run, jobs, artifacts, changed, repository)
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as target:
        target.write(f"run_id={run_id}\nsource_sha={run['head_sha']}\n")
    print(f"Verified four full/light platform artifacts from {run_id}; application inputs unchanged.")

if __name__ == '__main__':
    main()
