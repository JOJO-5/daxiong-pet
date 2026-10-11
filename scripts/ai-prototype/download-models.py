#!/usr/bin/env python3
"""Download the three pinned G1 text candidates and verify exact bytes/SHA-256."""
import argparse,concurrent.futures,hashlib,json,time,urllib.request
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--models',required=True);args=parser.parse_args()
root=Path(args.models);root.mkdir(parents=True,exist_ok=True)
manifest=Path(__file__).resolve().parents[2]/'docs/research/mobile-model-candidates-2026-10-10.json'
chosen=[x for x in json.loads(manifest.read_text())['candidates'] if x['repository'] in ['LiquidAI/LFM2.5-350M-GGUF','lmstudio-community/Qwen3-0.6B-GGUF']]
def fetch(x):
 target=root/x['name'];partial=target.with_suffix('.part');start=time.monotonic()
 try:
  if not target.exists():
   url=f"https://huggingface.co/{x['repository']}/resolve/{x['revision']}/{x['name']}"
   with urllib.request.urlopen(url,timeout=90) as response,partial.open('wb') as f:
    while chunk:=response.read(1024*1024):f.write(chunk)
   partial.rename(target)
  h=hashlib.sha256()
  with target.open('rb') as f:
   while chunk:=f.read(1024*1024):h.update(chunk)
  if target.stat().st_size!=x['bytes'] or h.hexdigest()!=x['sha256']:raise ValueError('size or hash mismatch; file not activated')
  return {'name':x['name'],'verified':True,'bytes':target.stat().st_size,'sha256':h.hexdigest(),'seconds':round(time.monotonic()-start,2)}
 except Exception as e:return {'name':x['name'],'verified':False,'error_type':type(e).__name__,'http_status':getattr(e,'code',None)}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(fetch,chosen))
(root/'download-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results,indent=2))
raise SystemExit(0 if all(r['verified'] for r in results) else 1)
