# ML Drift / LiteRT GPU 适配调研

核对日期：2026-10-11。用户新增要求：核对 Google 开源跨平台 GPU 推理能否用于大熊。结论：可以作为可选后端候选；不能直接替换现有 GGUF 运行器，也不能依据框架支持表宣称所有显卡已验证。

## 名称与支持范围

ML Drift 是 Google LiteRT 的 GPU 加速组件，源码在 `google-ai-edge/LiteRT/ml_drift_delegate`；语言模型通常使用 LiteRT-LM 的会话、分词、KV/循环状态与多模态编排。不是把 GPU 委托直接接到 llama.cpp 的 GGUF 上。

[LiteRT 官方说明](https://github.com/google-ai-edge/LiteRT/blob/aeae743070b3be22533261e4fa5bd3342631f8ee/README.md)列出 Windows/Linux WebGPU、macOS WebGPU/Metal、Android OpenCL/OpenGL、iOS Metal。源码包含 WebGPU、Metal、OpenCL、Vulkan 加速接口，但接口存在不等于每种平台/驱动/模型组合都发布并验收。WebGPU 可由底层后端对接不同厂商 GPU，实际仍受驱动、算子、精度和内存限制。

[LiteRT-LM v0.18.0](https://github.com/google-ai-edge/LiteRT-LM/releases/tag/v0.18.0)提供 CPU/GPU 选择与 C++ API。原生构建可放在现有独立进程边界中，不必给用户安装 Python。GPU 分发还需要平台动态库；Windows 文档要求 DirectXShaderCompiler。Intel macOS 必须单独核对，不能从 Apple Silicon 的预编译包推断。

## 当前千问模型是否适用

[官方 Qwen3.5 转换说明](https://github.com/google-ai-edge/LiteRT-LM/blob/03d3979f8d6091a8357b122ac8148c231c01f362/models/qwen3_5/README.md)和 litert-torch 已提供 Qwen3.5 混合注意力转换路径。输入为原始 Hugging Face checkpoint，输出 `.litertlm`，不是现有 Q4_K_M GGUF。

[LiteRT Community 2B 模型](https://huggingface.co/litert-community/Qwen3.5-2B/tree/b6283ba1d52edf9eb3a6d77b4288c6197e9eab42)固定 revision 的 API 元数据：

| 文件 | 实际字节 | SHA-256 |
| --- | ---: | --- |
| Qwen3.5-2B_int8.litertlm | 2,116,592,816 | 8ae5027136e0b89039ed76eedfb61f0cf4b7277024d99bf765359264295b86b1 |
| Qwen3.5-2B-VL_int8.litertlm | 3,148,905,040 | f8821e403dcfc939a033f6b2c9633a5d6b78c29a380ca825e0b90a62df61afe5 |

许可证元数据 Apache-2.0。文本版本约为当前 1,270,808,032 字节 GGUF 的 1.67 倍；视觉版本约为当前 GGUF+projector 1,942,180,448 字节的 1.62 倍。不能为了跨厂商 GPU 就说包更小。转换 INT4 是值得试验的另一个方案，需要重新评估转换一致性和显卡算子，不能把现成 INT8 的结果套到 INT4。

模型卡由社区维护者提供，报告 M4 Max GPU 解码较快，同时明确记录 Pixel 8a 8GB GPU 内存不足、部分复杂提示 CPU INT8 质量下降、视觉固定512×512和位置编码差异。它们是第三方数据；本项目尚未下载/运行 `.litertlm`，没有 Windows i5 核显、NVIDIA、AMD、Intel Mac 的实测结果。不以手机/M4 性能预测用户的 8GB 11代 i5。

## 在项目中的实施决定

现阶段继续完成已验证的 llama.cpp CPU、原生 OCR、屏幕范围和数据功能；将 ML Drift/LiteRT-LM 作为 G6 可选加速试验，与 llama.cpp Vulkan/Metal 比较，保留同一任务取消、短期画面、凭据和 CPU 回退接口。

资格试验按以下顺序进行：

1. 固定稳定运行器、模型格式、revision、哈希、完整许可证，确认四个发布目标可构建；先验证 CPU 输出和多轮状态。
2. 实际测 Windows Intel 核显/独显、macOS Apple Silicon；Intel Mac/Linux 分别记录可用性。采集冷启动、首句、解码、进程树内存、驱动与运行器增量大小。
3. 用现有中文陪伴/明确偏好/图片样本复验转换后的质量；特别核对 Qwen3.5 GatedDeltaNet 和视觉处理，而不是只运行 Gemma 示例。
4. 在拖动/球/飞盘同时运行时测干扰，注入无显卡、初始化失败、内存不足、进程崩溃和取消；GPU 失败只回退本地 CPU，不自动换云端。
5. 通过后才提供可选加速组件。8GB 默认是否启用必须由实际剩余内存及测量决定；不在默认包同时塞两份完整权重。

当前无可用于硬件验证的 `/dev/dri` GPU 设备，因此本次是来源与适配调查，不是 GPU 性能验收。
