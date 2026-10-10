#!/usr/bin/env python3
"""Stage a verified CPU runner/model outside git for native development or Tauri resources.
No network download, no activation of unverified weights, no overwriting an existing stage.
"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for data in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(data)
    return digest.hexdigest()
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime-dir', required=True)
    parser.add_argument('--model', required=True)
    parser.add_argument('--runtime-license', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--symlink', action='store_true', help='Development only; never distribute links to local source files.')
    parser.add_argument('--bundle-config', help='Write a separate Tauri resource override, for a copy stage only.')
    args = parser.parse_args()
    stage = Path(args.out).resolve()
    if stage.exists() or (args.bundle_config and Path(args.bundle_config).exists()):
        parser.error('output must not already exist')
    if args.symlink and args.bundle_config:
        parser.error('bundle requires a copy stage, not development symlinks')
    spec = json.loads((ROOT / 'scripts/ai-prototype/default-model.json').read_text())['default_text_model']
    model = Path(args.model).resolve()
    if model.name != spec['name'] or model.stat().st_size != spec['bytes'] or sha(model) != spec['sha256']:
        parser.error('model does not match pinned Qwen3.5 2B artifact')
    runtime = Path(args.runtime_dir).resolve()
    binaries = [p for p in runtime.iterdir() if p.name in ('llama-server', 'llama-server.exe')]
    if len(binaries) != 1:
        parser.error('runtime directory must contain exactly one platform llama-server executable')
    license_file = Path(args.runtime_license).resolve()
    if not license_file.is_file():
        parser.error('runtime license is required')
    files = [p for p in runtime.iterdir() if p.is_file() and (p in binaries or ('bench' not in p.name and ('.so' in p.name or p.suffix in ('.dll', '.dylib'))))]
    (stage / 'runtime').mkdir(parents=True)
    (stage / 'models').mkdir()
    (stage / 'licenses').mkdir()
    for source, target in [(p, stage / 'runtime' / p.name) for p in files] + [(model, stage / 'models' / spec['name'])]:
        if args.symlink:
            target.symlink_to(source.resolve())
        elif source.is_symlink():
            # Only preserve links contained in the supplied runtime directory.
            linked = source.resolve()
            if linked.parent != runtime:
                parser.error('runtime symlink points outside supplied directory')
            shutil.copy2(linked, target)
        else:
            shutil.copy2(source, target)
    shutil.copy2(license_file, stage / 'licenses/llama.cpp-LICENSE.txt')
    shutil.copy2(ROOT / 'scripts/ai-prototype/QWEN35-LICENSE.txt', stage / 'licenses/QWEN35-LICENSE.txt')
    manifest = {'model': spec, 'development_symlinks': args.symlink, 'runtime_commit_expected': json.loads((ROOT / 'scripts/ai-prototype/default-model.json').read_text())['runtime']['commit'], 'files': {str(p.relative_to(stage)): {'bytes': p.stat().st_size, 'sha256': sha(p)} for p in stage.rglob('*') if p.is_file()}}
    (stage / 'resource-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    if args.bundle_config:
        Path(args.bundle_config).write_text(json.dumps({'bundle': {'resources': {str(stage) + '/': 'ai/'}}}, indent=2) + '\n')
    print('Verified stage:', stage)
    print('Development: set DAXIONG_AI_RESOURCE_DIR to this directory before launching a debug app.')
    print('Text-only development slice; OCR and visual component are not staged by this helper yet.')
if __name__ == '__main__':
    main()
