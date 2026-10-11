# 原生 OCR worker 开发记录

2026-10-11（北京时间），按目标文件连续开发。独立 C++ worker 使用 ONNX Runtime CPU、OpenCV 图像处理和 Clipper；输入是 stdin 中的 PNG，输出是有界 JSON，不依赖 Python、不写截图文件。Rust监督器检查固定模型大小/哈希、CPU输出类型、坐标/置信度范围，支持30秒超时与取消后的kill/wait。

Linux Ubuntu22.04、GCC11.4、ONNX Runtime1.23.2和系统OpenCV4.5.4，已真实运行12张此前合成样本，全部正常退出；归一化后文字与Python参考一致，无文字图形/空白没有误报。[逐项结果](evidence/native-ocr-2026-10-11/linux-dynamic-reference.json)。此轮每图为新进程，不把启动加推理耗时当热运行延迟。

正式分发使用固定OpenCV4.12.0、仅core/imgproc/imgcodecs与PNG/zlib静态构建，减少动态依赖；固定四平台CPU运行器源见native/ocr/runtime-sources.json，原生验证工作流单独运行。本记录不代表新静态包、Rust监督器取消/超时或四个平台都已经通过，后续补实际结果。屏幕捕获和设置界面正在其他阶段接入；本轮不发布新版本。未改美术素材。
