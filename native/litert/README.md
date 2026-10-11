# Native LiteRT / ML Drift research component

Standalone Rust worker calling the LiteRT-LM **0.18.0 opaque C ABI**. It is not
linked into Tauri and does not need Python for inference. The present
qualification uses Qwen3.5 **0.8B INT8 text on CPU**, not the product's default
2B GGUF or a hardware GPU qualification.

The intended GPU is Google **ML Drift**, reached through LiteRT-LM's compiled
model GPU executor and LiteRT's accelerator. LiteRT-LM v0.18.0 pins LiteRT
`26895c9fbcc25c43faa8c1a98cd1fd28951602c3`; that revision has
`litert/runtime/accelerators/gpu/ml_drift_webgpu_accelerator.cc` and
`ml_drift_delegate/delegate:delegate_webgpu`. Native library symbol inspection
also finds `ml_drift`. Source/symbol presence is **not hardware execution**.

## Build and stage

```sh
cargo test --locked --manifest-path native/litert/Cargo.toml
cargo build --release --locked --manifest-path native/litert/Cargo.toml
python scripts/stage-litert-native.py --work litert-build --out litert-component \
  --worker native/litert/target/release/daxiong-litert-worker
# Windows uses daxiong-litert-worker.exe. Use a fresh output directory.
python scripts/smoke-litert-native.py --component litert-component \
  --model litert-build/Qwen3.5-0.8B_int8.litertlm --out test-results/litert
```

Python and psutil are build/test tools only. Staging downloads and verifies
fixed platform **libraries only**, without weights. The smoke test downloads
the separate fixed, verified research model. Mac SDK libraries are thinned to
the runner's actual architecture. The stage manifest records every file's
size/digest and the measurement excludes Python and model weights.

## Launch and protocol

```sh
./daxiong-litert-worker --library /absolute/path/liblitert-lm.so \
  --model /absolute/path/model.litertlm --backend cpu --threads 2 --context 1024
```

Library names vary by platform; read `component-manifest.json`. The worker
prints one JSON `ready` event containing its actual loopback port and PID.
HTTP binds **127.0.0.1 only**, supports `/health`, `/v1/models`, and streaming
`/v1/chat/completions` using model ID `daxiong-litert`. Text system/user/assistant
history is rebuilt per request. Thinking is disabled; `temperature` 0..2 and
output limits 1..256 are supported. The schema rejects media and tools instead
of silently omitting them. Browser origins, ambiguous lengths and oversized
requests are rejected. Only one inference is admitted; another returns 409.

The owner keeps stdin open and sends newline-delimited controls:

```json
{"op":"cancel"}
{"op":"shutdown"}
```

EOF also shuts down. Controls are not public HTTP endpoints. Disconnecting an
SSE client cancels its generation; a fresh request constructs a new conversation
using the supplied history. Callback data remains alive through the native
terminal callback and callback return. A 90-second inference deadline and
5-second cancellation/shutdown watchdog terminate a stuck **worker process**
instead of allowing a native callback to use freed memory. The parent must
also enforce startup deadlines and handle abnormal exits.

For real Tauri research integration run `scripts/research/e2e-litert-native.py`
via `scripts/e2e-linux.sh` with `LITERT_COMPONENT`, `LITERT_TEST_MODEL`, and
`E2E_SCRIPT`. This exercises the existing external backend in isolated test
settings; it does not install the component for a normal user.

## Optional download contract

This is intended as an **optional download**, never a second default model
bundle. The platform component and compatible `.litertlm` weights must be
downloaded separately with visible sizes, progress, cancellation, atomic
staging and SHA-256 verification. GPU initialization/errors must fall back to
the existing local CPU runner, never implicitly send data to a cloud service.

The product GUI downloader, managed process integration/automatic CPU fallback,
2B INT4 conversion and hardware GPU qualification are subsequent work. Research
payload metrics are not final installer increments. Final runtime notices,
dependency inventory, signing and distribution checks remain product gates.
CI uploads the wrapper and evidence **without redistributing Google's SDK**;
it measures the runtime-containing archive locally. No application release/tag
is produced by this workflow.
