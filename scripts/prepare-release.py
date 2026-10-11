#!/usr/bin/env python3
"""Give the four validated platform artifacts stable public download names."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tomllib

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--artifacts', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--bundled-ai', action='store_true', help='Installers carry resources; do not publish a resource-less portable exe')
args = parser.parse_args()
version = json.loads((root/'package.json').read_text())['version']
assert json.loads((root/'package-lock.json').read_text())['version'] == version
assert json.loads((root/'src-tauri/tauri.conf.json').read_text())['version'] == version
assert tomllib.loads((root/'src-tauri/Cargo.toml').read_text())['package']['version'] == version
lock = tomllib.loads((root/'src-tauri/Cargo.lock').read_text())
assert next(p for p in lock['package'] if p['name']=='daxiong-pet')['version'] == version

packages = [
    ('daxiong-windows-x64', '*-setup.exe', 'windows-x64-setup.exe'),
    ('daxiong-windows-x64', 'daxiong-pet.exe', 'windows-x64-portable.exe'),
    ('daxiong-macos-arm64', '*.dmg', 'macos-arm64.dmg'),
    ('daxiong-macos-x64', '*.dmg', 'macos-x64.dmg'),
    ('daxiong-linux-x64', '*.AppImage', 'linux-x64.AppImage'),
    ('daxiong-linux-x64', '*.deb', 'linux-x64.deb'),
]
if args.bundled_ai:
    packages = [p for p in packages if p[2] != "windows-x64-portable.exe"]
resolved=[]
for artifact, pattern, suffix in packages:
    found = list((args.artifacts/artifact).rglob(pattern))
    assert len(found)==1, f'{artifact}/{pattern}: expected one package, found {found}'
    assert found[0].stat().st_size>0, f'empty package: {found[0]}'
    resolved.append((found[0],f'Daxiong-{version}-{suffix}'))
# Resolve all inputs before creating any output; never publish a partial set.
args.out.mkdir(parents=True,exist_ok=True)
checksums=[]
for source, name in resolved:
    target=args.out/name
    shutil.copyfile(source,target)
    with target.open("rb") as file:
        digest=hashlib.file_digest(file,"sha256").hexdigest()
    checksums.append(f'{digest}  {name}')
    print(name, target.stat().st_size)
(args.out/'SHA256SUMS').write_text('\n'.join(checksums)+'\n')
