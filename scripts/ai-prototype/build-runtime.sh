#!/usr/bin/env bash
set -euo pipefail
# Requires git, CMake >=3.14, a C++ compiler and make; no GPU SDK.
trial_root="${1:?Usage: build-runtime.sh /absolute/path/to/trial-workspace}"
mkdir -p "$trial_root"
git clone --depth 1 --branch v0.6.0 https://github.com/ggml-org/llama.cpp.git "$trial_root/llama.cpp"
test "$(git -C "$trial_root/llama.cpp" rev-parse HEAD)" = d81235049384534c167caea52b85a694f6103d14
cmake -S "$trial_root/llama.cpp" -B "$trial_root/runtime/build" \
  -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=OFF -DGGML_AVX2=ON \
  -DGGML_AVX512=OFF -DGGML_FMA=ON -DGGML_F16C=ON \
  -DGGML_CUDA=OFF -DGGML_VULKAN=OFF -DGGML_BLAS=OFF \
  -DGGML_OPENMP=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_TOOLS=ON -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_MTMD=OFF \
  -DLLAMA_OPENSSL=OFF -DGGML_CCACHE=OFF
cmake --build "$trial_root/runtime/build" --target llama-server llama-bench -j 2
