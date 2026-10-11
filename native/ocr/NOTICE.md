# Native OCR sources

CPU worker preprocessing follows RapidOCR v3.10.0 and PaddleOCR (Apache-2.0). The implementation performs DB detection, Clipper round offset and CTC decoding with the model-embedded dictionary.

Vendored Clipper 6.4.2 sources: fonttools/pyclipper tag 1.3.0.post6, `src/clipper.cpp` and `src/clipper.hpp`; Boost Software License and copyright remain in both headers. Vendored JSON: nlohmann/json v3.11.3, MIT, license retained.

OpenCV 4.12.0 source and ONNX Runtime 1.23.2 CPU archives have pinned SHA-256 in runtime-sources.json. Runtime bundling copies upstream and third-party notices. OCR converted weights are the fixed RapidAI/RapidOCR v3.10.0 ONNX files selected in docs/research/next-ai-integration-2026-10-11/models.json. The recognition dictionary is embedded in the ONNX, not an independently downloaded default dictionary.
