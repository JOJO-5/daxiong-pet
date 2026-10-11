#!/usr/bin/env python3
"""Derive a no-LLM-weight bundle from the verified full resource stage."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--source', required=True, type=Path)
p.add_argument('--out', required=True, type=Path)
p.add_argument('--config', required=True, type=Path)
a = p.parse_args()
source = a.source.resolve()
out = a.out.resolve()
assert not out.exists() and not a.config.exists(), 'outputs must be new'
manifest = json.loads((source / 'resource-manifest.json').read_text())
assert not manifest['development_symlinks'], 'distribution cannot contain dev links'
files = {}
for name, expected in manifest['files'].items():
    if name.startswith('models/'):
        continue
    src = (source / name).resolve()
    assert src.is_relative_to(source) and src.is_file()
    with src.open('rb') as f:
        digest = hashlib.file_digest(f, 'sha256').hexdigest()
    assert src.stat().st_size == expected['bytes'] and digest == expected['sha256'], name
    dst = out / name
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    files[name] = expected
assert any(n.startswith('runtime/llama-server') for n in files)
assert 'ocr/manifest.json' in files
manifest.update(files=files, distribution='light-no-chat-or-vision-weights')
(out / 'resource-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
a.config.write_text(json.dumps({'bundle': {'resources': {str(out) + '/': 'ai/'}}}, indent=2) + '\n')
print('Verified light stage:', out, 'bytes:', sum(f['bytes'] for f in files.values()))
