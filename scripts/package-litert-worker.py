#!/usr/bin/env python3
"""Package only our verified native worker; Google SDKs/weights stay upstream."""
import argparse, hashlib, json, shutil, tarfile, io, tomllib, urllib.request, zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--artifacts',type=Path,required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--tag',required=True);a=p.parse_args()
a.out.mkdir(parents=True,exist_ok=True)
catalog={}
repo=Path(__file__).resolve().parents[1]
notices={'licenses/daxiong-LICENSE.txt':(repo/'LICENSE').read_bytes()}
lock=tomllib.loads((repo/'native/litert/Cargo.lock').read_text())
for dep in lock['package']:
    if 'checksum' not in dep:continue
    url=f"https://static.crates.io/crates/{dep['name']}/{dep['name']}-{dep['version']}.crate"
    with urllib.request.urlopen(url,timeout=60) as response:data=response.read()
    assert hashlib.sha256(data).hexdigest()==dep['checksum']
    found=0
    with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as archive:
        for member in archive.getmembers():
            if member.isfile() and ('license' in Path(member.name).name.lower() or Path(member.name).name.lower() in ('copying','copyright')):
                notices['licenses/'+member.name]=archive.extractfile(member).read();found+=1
    assert found,('license files missing',dep['name'])

for target in ('linux-x64','windows-x64','macos-arm64','macos-x64'):
    root=a.artifacts/('litert-native-'+target)
    results=list(root.rglob('native-smoke.json'));assert len(results)==1
    result=json.loads(results[0].read_text());assert result['status']=='passed' and len(result['checks'])>=24,(target,result)
    binaries=list(root.rglob('daxiong-litert-worker.exe' if target.startswith('windows') else 'daxiong-litert-worker'));assert len(binaries)==1
    source=binaries[0];name=f'daxiong-litert-worker-{target}'+('.exe' if target.startswith('windows') else '')
    name=f'daxiong-litert-worker-{target}.zip'
    dst=a.out/name
    with zipfile.ZipFile(dst,'w',compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(source,'daxiong-litert-worker'+('.exe' if target.startswith('windows') else ''))
        for legal,data in notices.items():archive.writestr(legal,data)
    catalog[target]={'url':f'https://github.com/JOJO-5/daxiong-pet/releases/download/{a.tag}/{name}','bytes':dst.stat().st_size,'sha256':hashlib.sha256(dst.read_bytes()).hexdigest(),'name':'daxiong-litert-worker'+('.exe' if target.startswith('windows') else '')}
(a.out/'worker-catalog.json').write_text(json.dumps({'tag':a.tag,'workers':catalog},indent=2)+'\n')
shutil.copyfile(Path(__file__).resolve().parents[1]/'LICENSE',a.out/'daxiong-litert-worker-LICENSE.txt')
