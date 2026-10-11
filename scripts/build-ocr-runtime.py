#!/usr/bin/env python3
"""Build a small native OCR worker with static PNG-only OpenCV and CPU ORT."""
import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tarfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fetch(spec, path):
    if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest() == spec['sha256']:
        return path
    partial = path.with_suffix('.part')
    digest = hashlib.sha256()
    count = 0
    with urllib.request.urlopen(spec['url'], timeout=120) as response, partial.open('wb') as target:
        while data := response.read(1024 * 1024):
            target.write(data)
            digest.update(data)
            count += len(data)
    if count != spec['bytes'] or digest.hexdigest() != spec['sha256']:
        raise RuntimeError('OCR dependency checksum mismatch: ' + path.name)
    partial.replace(path)
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cmake', default='cmake')
    parser.add_argument('--jobs', type=int, default=2)
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    sources = json.loads((ROOT / 'native/ocr/runtime-sources.json').read_text())
    suffix = {'Linux': 'linux-x64', 'Windows': 'win-x64', 'Darwin': 'osx-arm64' if platform.machine() == 'arm64' else 'osx-x86_64'}[platform.system()]
    ort = next(s for s in sources['onnxruntime'] if suffix + '-1.23.2.' in s['name'])
    archive = fetch(ort, work / ort['name'])
    if archive.suffix == '.zip':
        with zipfile.ZipFile(archive) as z:
            for member in z.infolist():
                if not (work / member.filename).resolve().is_relative_to(work):
                    raise RuntimeError('invalid archive path')
            z.extractall(work)
    else:
        with tarfile.open(archive) as t:
            t.extractall(work, filter='data')
    ort_root = work / ort['name'].removesuffix('.tgz').removesuffix('.zip')
    opencv = fetch(sources['opencv'], work / 'opencv-4.12.0.tar.gz')
    if not (work / 'opencv-4.12.0').exists():
        with tarfile.open(opencv) as t:
            t.extractall(work, filter='data')
    install = work / 'opencv-install'
    command = [args.cmake, '-S', str(work / 'opencv-4.12.0'), '-B', str(work / 'opencv-build'), '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_INSTALL_PREFIX=' + str(install), '-DBUILD_LIST=core,imgproc,imgcodecs', '-DBUILD_SHARED_LIBS=OFF', '-DBUILD_TESTS=OFF', '-DBUILD_PERF_TESTS=OFF', '-DBUILD_EXAMPLES=OFF', '-DBUILD_opencv_apps=OFF', '-DBUILD_opencv_gapi=OFF', '-DWITH_ADE=OFF', '-DWITH_VTK=OFF', '-DBUILD_JAVA=OFF', '-DBUILD_opencv_python2=OFF', '-DBUILD_opencv_python3=OFF', '-DWITH_PNG=ON', '-DBUILD_PNG=ON', '-DBUILD_ZLIB=ON']
    for feature in ['JPEG', 'TIFF', 'WEBP', 'OPENJPEG', 'JASPER', 'OPENEXR', 'GDAL', 'FFMPEG', 'GSTREAMER', 'GTK', 'QT', 'IPP', 'ITT', 'OPENCL', 'LAPACK', 'EIGEN', 'PROTOBUF', 'TBB', 'V4L', 'DC1394', 'AVFOUNDATION', 'MSMF']:
        command.append('-DWITH_' + feature + '=OFF')
    subprocess.run(command, check=True)
    subprocess.run([args.cmake, '--build', str(work / 'opencv-build'), '--config', 'Release', '--parallel', str(args.jobs)], check=True)
    subprocess.run([args.cmake, '--install', str(work / 'opencv-build'), '--config', 'Release'], check=True)
    build = work / 'worker'
    subprocess.run([args.cmake, '-S', str(ROOT / 'native/ocr'), '-B', str(build), '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_PREFIX_PATH=' + str(install), '-DORT_ROOT=' + str(ort_root)], check=True)
    subprocess.run([args.cmake, '--build', str(build), '--config', 'Release', '--parallel', str(args.jobs)], check=True)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run([args.cmake, '--install', str(build), '--config', 'Release', '--prefix', str(output)], check=True)
    for p in (ort_root / 'lib').iterdir():
        if p.is_file() and ('.so' in p.name or '.dylib' in p.name or p.suffix == '.dll'):
            shutil.copyfile(p, output / p.name)
    notices = output / 'licenses'
    notices.mkdir(exist_ok=True)
    for name, source in [('onnxruntime-LICENSE', ort_root / 'LICENSE'), ('onnxruntime-ThirdPartyNotices.txt', ort_root / 'ThirdPartyNotices.txt'), ('OpenCV-LICENSE', work / 'opencv-4.12.0/LICENSE'), ('RapidOCR-LICENSE', ROOT / 'docs/research/qwen35-2b-ocr-route-2026-10-10/RapidOCR-LICENSE.txt'), ('json-LICENSE.MIT', ROOT / 'native/ocr/vendor/json-LICENSE.MIT'), ('clipper-license-and-header.txt', ROOT / 'native/ocr/vendor/clipper.hpp')]:
        shutil.copyfile(source, notices / name)
    # Retain the bundled PNG/zlib notices without editing their text.
    for directory in ['libpng', 'zlib']:
        source = work / 'opencv-4.12.0/3rdparty' / directory
        for name in ['LICENSE', 'README', 'README.md']:
            if (source / name).is_file():
                shutil.copyfile(source / name, notices / (directory + '-' + name))
    models = json.loads((ROOT / 'docs/research/next-ai-integration-2026-10-11/models.json').read_text())
    for spec in models:
        if spec['key'].startswith('ch_PP-OCRv5'):
            fetch(spec, output / (spec['key'] + '.onnx'))
    manifest = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest() for p in output.rglob('*') if p.is_file() and p.name != 'manifest.json'}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()
