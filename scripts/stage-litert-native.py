#!/usr/bin/env python3
"""Prepare a fixed, isolated native research component. Does NOT install AI or
download model weights. Python is a build tool, not a runtime dependency.
Research package bytes are measured locally; runtime redistribution/license
qualification is separate from the wrapper-only CI artifact.
"""
import argparse,hashlib,json,platform,shutil,subprocess,sys,urllib.request,zipfile
from pathlib import Path
REPO=Path(__file__).resolve().parents[1]
SOURCES=json.loads((REPO/'native/litert/runtime-sources.json').read_text())

def sha(path):
    with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def download(spec,target):
    target.parent.mkdir(parents=True,exist_ok=True)
    if target.exists() and target.stat().st_size==spec['bytes'] and sha(target)==spec['sha256']:return target
    if target.exists():raise RuntimeError('Existing download failed fixed digest: '+str(target))
    partial=target.with_suffix(target.suffix+'.part')
    req=urllib.request.Request(spec['url'],headers={'User-Agent':'Daxiong-LiteRT-native-research'})
    with urllib.request.urlopen(req,timeout=60) as response,partial.open('wb') as out:
        for chunk in iter(lambda:response.read(1024*1024),b''):out.write(chunk)
    assert partial.stat().st_size==spec['bytes'],'download size mismatch'
    assert sha(partial)==spec['sha256'],'download digest mismatch'
    partial.replace(target);return target

def target_name():
    machine=platform.machine().lower()
    if sys.platform=='win32' and machine in ['amd64','x86_64']:return 'windows-x64'
    if sys.platform=='linux' and machine=='x86_64':return 'linux-x64'
    if sys.platform=='darwin' and machine in ['arm64','x86_64']:return 'macos-'+('arm64' if machine=='arm64' else 'x64')
    raise RuntimeError('Unsupported native target '+sys.platform+'/'+machine)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--worker',type=Path,required=True)
    args=parser.parse_args();target=target_name();spec=SOURCES[target]
    out=args.out.resolve();out.mkdir(parents=True,exist_ok=True)
    if list(out.iterdir()):raise RuntimeError('Use a fresh component output directory')
    source=download(spec,args.work.resolve()/('runtime-'+target+'.zip'))
    with zipfile.ZipFile(source) as archive:
        for filename in spec['files']:
            candidates=[n for n in archive.namelist() if Path(n).name==filename]
            assert len(candidates)==1,(filename,candidates)
            destination=out/filename
            with archive.open(candidates[0]) as src,destination.open('wb') as dst:shutil.copyfileobj(src,dst)
            if spec.get('thin'):
                thin=out/(filename+'.thin')
                subprocess.run(['lipo',str(destination),'-thin',spec['thin'],'-output',str(thin)],check=True)
                thin.replace(destination)
    worker=out/('daxiong-litert-worker.exe' if sys.platform=='win32' else 'daxiong-litert-worker')
    shutil.copy2(args.worker,worker)
    runtime=out/spec['files'][0]
    probe=subprocess.run([str(worker),'--library',str(runtime),'--probe'],cwd=out,capture_output=True,text=True,timeout=20)
    (args.work/'native-probe.log').write_text(probe.stdout+probe.stderr)
    assert probe.returncode==0,'native ABI probe failed'
    manifest={'component':'litert-native-research','version':SOURCES['version'],'target':target,'protocol':'OpenAI SSE + parent-owned stdin controls','optional_download':True,'default_package':False,'python_runtime_required':False,'model_included':False,'redistribution_status':'research only; final license inventory/signing pending','source':spec,'files':{p.name:{'bytes':p.stat().st_size,'sha256':sha(p)} for p in out.iterdir() if p.is_file()}}
    (out/'component-manifest.json').write_text(json.dumps(manifest,indent=2))
    package=args.work.resolve()/('component-'+target+'.zip')
    with zipfile.ZipFile(package,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as archive:
        for p in sorted(out.iterdir()):archive.write(p,p.name)
    metrics={'target':target,'sdk_version':SOURCES['version'],'native_probe':json.loads(probe.stdout.strip().splitlines()[-1]),'worker_bytes':worker.stat().st_size,'runtime_files_bytes':sum(manifest['files'][n]['bytes'] for n in spec['files']),'component_zip_bytes':package.stat().st_size,'zip_scope':'native worker + required runtime files + manifest; no model, Python, final notices or installer overhead; measured research payload not a qualified product download'}
    (args.work/'component-metrics.json').write_text(json.dumps(metrics,indent=2));print(json.dumps(metrics),flush=True)

if __name__=='__main__':main()
