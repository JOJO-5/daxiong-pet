#!/usr/bin/env python3
"""Build only the pinned CPU server. Run on the target OS/toolchain, not a newer host."""
import argparse
import json
import platform
import subprocess
from pathlib import Path

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', required=True)
    parser.add_argument('--build-dir', required=True)
    parser.add_argument('--cmake', default='cmake')
    parser.add_argument('--jobs', type=int, default=2)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 16:
        parser.error('jobs must be between 1 and 16')
    root = Path(__file__).resolve().parents[1]
    spec = json.loads((root / 'scripts/ai-prototype/default-model.json').read_text())
    source = Path(args.source).resolve()
    commit = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    if commit != spec['runtime']['commit']:
        parser.error('source commit does not match the pinned runtime')
    if subprocess.check_output(['git', '-C', str(source), 'status', '--porcelain', '--untracked-files=no'], text=True).strip():
        parser.error('tracked runtime source must be clean')
    machine = platform.machine().lower()
    if machine not in ('x86_64', 'amd64', 'aarch64', 'arm64'):
        parser.error('unsupported development architecture')
    flags = ['-DCMAKE_BUILD_TYPE=Release', '-DGGML_NATIVE=OFF', '-DGGML_CUDA=OFF', '-DGGML_VULKAN=OFF', '-DGGML_METAL=OFF', '-DGGML_BLAS=OFF', '-DGGML_OPENMP=OFF', '-DGGML_CCACHE=OFF', '-DLLAMA_BUILD_TESTS=OFF', '-DLLAMA_BUILD_EXAMPLES=OFF', '-DLLAMA_BUILD_TOOLS=ON', '-DLLAMA_BUILD_SERVER=ON', '-DLLAMA_BUILD_MTMD=OFF', '-DLLAMA_OPENSSL=OFF', '-DLLAMA_BUILD_UI=OFF', '-DLLAMA_USE_PREBUILT_UI=OFF', '-DLLAMA_BUILD_COMMIT=' + commit[:12]]
    if machine in ('x86_64', 'amd64'):
        flags += ['-DGGML_AVX2=ON', '-DGGML_AVX512=OFF', '-DGGML_FMA=ON', '-DGGML_F16C=ON']
    build = str(Path(args.build_dir).resolve())
    subprocess.run([args.cmake, '-S', str(source), '-B', build, *flags], check=True)
    subprocess.run([args.cmake, '--build', build, '--config', 'Release', '--target', 'llama-server', '-j', str(args.jobs)], check=True)
    print('Built pinned CPU server; stage the bin directory with stage-ai-resources.py.')
    print('x86 build requires AVX2/FMA/F16C. Build success is not native Windows/macOS validation.')

if __name__ == '__main__':
    main()
