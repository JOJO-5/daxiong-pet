#!/usr/bin/env python3
"""Fetch the pinned text model, build native CPU runtime, and stage real bundle resources."""
import argparse, hashlib, json, subprocess, urllib.request, sys
from pathlib import Path
root = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--work', required=True, type=Path)
a = p.parse_args(); work = a.work.resolve(); work.mkdir(parents=True, exist_ok=True)
spec = json.loads((root/'scripts/ai-prototype/default-model.json').read_text())
source = work/'llama.cpp'
if not source.exists(): subprocess.run(['git','clone','--no-checkout','https://github.com/ggml-org/llama.cpp.git',str(source)],check=True)
subprocess.run(['git','-C',str(source),'checkout','--detach',spec['runtime']['commit']],check=True)
subprocess.run(['python',str(root/'scripts/build-ai-runtime.py'),'--source',str(source),'--build-dir',str(work/'build')],check=True)
def fetch_model(m):
    model = work/m['name']; partial = model.with_suffix('.part')
    if model.is_file() and model.stat().st_size == m['bytes']:
        digest = hashlib.sha256()
        with model.open('rb') as existing:
            while data := existing.read(1024*1024): digest.update(data)
        if digest.hexdigest() == m['sha256']: return model
    url = 'https://huggingface.co/'+m['repository']+'/resolve/'+m['revision']+'/'+m['name']
    digest = hashlib.sha256(); size = 0
    with urllib.request.urlopen(url,timeout=120) as response, partial.open('wb') as out:
        while data := response.read(1024*1024):
            out.write(data); digest.update(data); size += len(data)
    if size != m['bytes'] or digest.hexdigest() != m['sha256']: raise RuntimeError('model checksum mismatch')
    partial.replace(model)
    return model
model = fetch_model(spec['default_text_model'])
projector = fetch_model(spec['vision_component'])
ocr = work/'ocr'
subprocess.run([sys.executable,str(root/'scripts/build-ocr-runtime.py'),'--work',str(work/'ocr-build'),'--out',str(ocr)],check=True)
subprocess.run([sys.executable,str(root/'scripts/smoke-ocr-runtime.py'),'--ocr',str(ocr),'--out',str(work/'ocr-smoke')],check=True)
runtime = work/'build/bin'
if (runtime/'Release/llama-server.exe').exists(): runtime = runtime/'Release'
# Static CPU runner must work after its build tree disappears.
isolated = work/'portable-runtime'; isolated.mkdir(exist_ok=True)
import shutil
executable = runtime/('llama-server.exe' if __import__('os').name=='nt' else 'llama-server')
shutil.copy2(executable, isolated/executable.name)
subprocess.run([str(isolated/executable.name),'--version'],check=True)
runtime = isolated
subprocess.run(['python',str(root/'scripts/stage-ai-resources.py'),'--runtime-dir',str(runtime),'--model',str(model),'--projector',str(projector),'--ocr-dir',str(ocr),'--runtime-license',str(source/'LICENSE'),'--out',str(work/'ai'),'--bundle-config',str(work/'tauri-ai.json')],check=True)
