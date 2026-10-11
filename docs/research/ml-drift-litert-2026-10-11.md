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

模型卡由社区维护者提供，报告 M4 Max GPU 解码较快，同时明确记录 Pixel 8a 8GB GPU 内存不足、部分复杂提示 CPU INT8 质量下降、视觉固定512×512和位置编码差异。它们是第三方数据；首次来源调查时尚未下载/运行 `.litertlm`（之后的CPU试验见下文）；没有 Windows i5 核显、NVIDIA、AMD、Intel Mac 的实测结果。不以手机/M4 性能预测用户的 8GB 11代 i5。

## 在项目中的实施决定

现阶段继续完成已验证的 llama.cpp CPU、原生 OCR、屏幕范围和数据功能；将 ML Drift/LiteRT-LM 作为 G6 可选加速试验，与 llama.cpp Vulkan/Metal 比较，保留同一任务取消、短期画面、凭据和 CPU 回退接口。

资格试验按以下顺序进行：

1. 固定稳定运行器、模型格式、revision、哈希、完整许可证，确认四个发布目标可构建；先验证 CPU 输出和多轮状态。
2. 实际测 Windows Intel 核显/独显、macOS Apple Silicon；Intel Mac/Linux 分别记录可用性。采集冷启动、首句、解码、进程树内存、驱动与运行器增量大小。
3. 用现有中文陪伴/明确偏好/图片样本复验转换后的质量；特别核对 Qwen3.5 GatedDeltaNet 和视觉处理，而不是只运行 Gemma 示例。
4. 在拖动/球/飞盘同时运行时测干扰，注入无显卡、初始化失败、内存不足、进程崩溃和取消；GPU 失败只回退本地 CPU，不自动换云端。
5. 通过后才提供可选加速组件。8GB 默认是否启用必须由实际剩余内存及测量决定；不在默认包同时塞两份完整权重。

当前无可用于硬件验证的 `/dev/dri` GPU 设备，因此本次包含来源调查与独立CPU试验，不是 GPU 性能验收。

## 独立CPU最小试验（本次用户允许试验）

安装官方PyPI `litert-lm==0.18.0`，未修改生产依赖或加入安装包。下载并校验社区[Qwen3.5 0.8B](https://huggingface.co/litert-community/Qwen3.5-0.8B/tree/10243c0a01d24e6b122a0e814f8c4d984c9c92d6)文字INT8：963,184,864字节，SHA256 `64ed396fcdae75e5158945c77a08142b1322ce1bbf1be4d2198783119a1169e8`。当前GGUF 0.8B为527,502,816字节；现成LiteRT INT8更大，不能证明INT4转换后仍有同样差距。官方Python轮内`liblitert-lm.so`未压缩131,785,976字节，此SDK文件大小不是定制C++发行版或所有平台的增量包大小。

指定`Backend.CPU(thread_count=2)`、上下文1024、输出32、`ThinkingConfig(enable_thinking=False)`，实际中文问候成功。共享Linux主机本轮加载28.837秒、回复3.288秒、Python进程峰值1,992,988KiB。首次误用API参数`enabled`失败，保留记录后按SDK实际字段修正；不删除失败。不以此对比2B模型或预测8GB/i5。GPU仅注册组件，实际选用CPU，未做GPU硬件推理。

官方CLI `serve`仅绑定127.0.0.1，独立配置CPU/2线程/1024上下文/关闭思考。通过本项目已有外接服务接口，真实原生Tauri聊天收到该模型SSE回复、落库并与宠物气泡内容一致，3项通过；包含首次加载的本轮整体70.113秒，不是仅解码速度。证据`/workspace/daxiong-ai-prototype/litert-research`：固定Hub元数据、下载哈希、最小脚本、SDK日志、CLI配置、原生回归及录屏。首张原生截图全聊天窗口遮住宠物，不能当作气泡视觉验收；后续独立布局重试有一次窗口尚未映射的测试失败，保留记录。第三轮等待原生窗口映射后通过同样3项检查，包含首次加载的整体61.264秒；实际查看截图，完整聊天窗口仍局部遮住气泡，因此不将其计作完整气泡视觉验收。

首次试验结论：LiteRT CPU可独立运行，已有OpenAI风格外接入口可用于试验，无需在默认包同时装两套权重。当时尚未验证Qwen3.5 2B/INT4转换、视觉、取消/崩溃/退出全链路、Windows/Mac或GPU；本轮新增的独立SDK/CLI生命周期验证见下文，产品接入与GPU实机仍未完成。

## 第二轮：发行包核对与生命周期试验

2026-10-11，用户在alpha.5发布后要求继续研究Google跨平台方案。本轮重新查询官方发行信息：LiteRT-LM最新稳定版仍为[v0.18.0](https://github.com/google-ai-edge/LiteRT-LM/releases/tag/v0.18.0)，LiteRT独立项目最新为[v2.3.0](https://github.com/google-ai-edge/LiteRT/releases/tag/v2.3.0)。两个项目版本不同；本轮沿用完整0.18.0运行环境，没有把2.3.0动态库混装进去。应用依赖、默认CPU后端、OCR和alpha.5安装包均未改变。

### 跨平台发行实际包含什么

核对[litert-lm-api 0.18.0的PyPI文件](https://pypi.org/pypi/litert-lm-api/0.18.0/json)，现有Windows x64、Linux x64/ARM64、Mac ARM64、Android ARM64/x64运行包；没有Mac x64 Python wheel。顶层`litert-lm`约97KB只是CLI包装，不能当成运行器体积。

实际下载并核验官方Mac原生SDK与Windows API wheel的SHA-256，再读取归档结构和二进制头：

| 核对对象 | 实际压缩字节 | 原生内容与结论 |
| --- | ---: | --- |
| Windows x64 API wheel | 26,403,380 | `litert-lm.dll`50,051,584字节；另含`dxcompiler.dll`17,951,808、`dxil.dll`1,503,304；有额外Intel OpenVINO派发DLL。只检查包，不代表Windows执行/GPU已验证。 |
| Mac原生双架构SDK | 45,143,303 | 实际Mach-O同时包含ARM64的65,516,208字节切片和x64的72,266,768字节切片。两者最低OS头为12.0，依赖系统Metal等框架；本项目最低macOS 14不因此调整。 |

Mac归档总解压大小137,921,386字节，不能把它当成单架构增量。Python缺少Intel wheel不代表原生SDK缺少Intel支持；发行物架构存在也不代表Intel Mac上的Qwen/GPU推理已经跑通。Windows压缩包已携带DX编译库，但产品分发还要核对其许可证和实际加载路径。

这些是SDK/运行包体积，不是大熊最终可选组件或安装器增量；后者还包括原生包装进程、所需库、许可证、压缩方式与签名，需要构建实测。由此可以确定：不能仅凭之前Linux Python共享库约132MB就断言每个平台都会增加同样大小，更不能把顶层97KB包装包当作完整推理运行器。

### 权重仍是主要体积成本

继续使用已校验0.8B INT8文字模型963,184,864字节，仅用于独立原型；它不是当前默认2B模型。现成2B INT8文字/视觉约2.12/3.15GB，当前GGUF不能直接由LiteRT-LM加载。

官方[Qwen3.5转换示例](https://github.com/google-ai-edge/LiteRT-LM/blob/v0.18.0/models/qwen3_5/README.md)明确给出`--quantization_recipe=dynamic_wi4_afp32`的INT4权重配方。因此INT4有官方转换入口；本轮没有转换2B，不能承诺产物大小、中文质量、混合注意力GPU算子或显存占用已经合格，也不能把INT8试验结果套给INT4。

### 本轮真实CPU试验

同一共享Linux环境，0.18.0、0.8B INT8、2线程、1024上下文、关闭思考。SDK试验六项通过：两次真实回复、第二轮提取合成昵称、流式首块、取消未结束的生成、同一引擎新建会话后继续生成、显式关闭会话/引擎。取消到流式线程结束0.109秒；这不是HTTP断线取消延迟或所有后端的保证。引擎加载33.077秒，两轮回复2.673/1.072秒，进程峰值2,072,076KiB；存在前轮缓存文件，没有把它归类为严格冷启动，也不能预测8GB/11代i5或比较不同模型的优劣。

另启动官方CLI独立进程，只监听127.0.0.1，实际SSE断开连接后再次请求成功；收到第三次请求的真实流式数据后向自有进程组注入SIGKILL，退出码-9，监听端口消失。六项通过。日志出现预期的BrokenPipeError，官方处理器会调用`cancel_process()`；本轮没有测HTTP取消截止时间，也没有把“杀进程”写成优雅退出。SDK关闭、SDK取消、HTTP断线和崩溃各自记录，不互相替代。

执行成功与语义质量分开：第一轮模型虽然记住昵称，却回复“我是小林”而混淆主人/宠物身份；取消后的SDK新会话复述了问候指令。恢复请求仍产生正文，所以生命周期检查通过，但这些回复不能算性格/对话质量通过。这里只含少量合成提示，不能作为整体错误率或证明小模型不能用于宠物。

本轮JSON、发行包架构和文件摘要见[独立证据](litert-round2-2026-10-11.json)。原始SDK、失败/断线日志及脚本在`/workspace/daxiong-ai-prototype/litert-research/round2`；试验结束自有CLI进程已退出。复现脚本为`scripts/research/litert-lifecycle-trial.py`和`scripts/research/litert-http-lifecycle-trial.py`。先在独立目录准备`venv`、固定模型及上一轮`serve-config.json`，将输出目录设为其子目录`round2`，分别用该venv的Python或系统Python运行脚本并传`--work <独立目录>/round2`。CLI探针固定4531端口，会先检查端口空闲并只清理自己创建的进程组。

### 接入决定和下一轮验收

LiteRT-LM可以继续推进为按需下载的可选加速后端。默认CPU包维持现有运行器和OCR；聊天、SQLite、性格编辑、小输入框及气泡继续共用现有服务接口。原生包装器使用C/C++ API，运行于独立进程，不让普通用户安装Python。Python CLI是研究工具，不作为最终分发方案。

实施顺序：

1. **原生CPU桥接**：锁0.18.0，编译Windows x64、Mac ARM/x64、Linux x64包装进程；先复现本轮多轮、流式、取消、关闭/崩溃行为，再测真正单平台压缩增量。现有Mac双架构发行物只完成结构核对，不算四平台执行验收。
2. **模型转换**：单独转换当前2B至INT4，固定原checkpoint、转换器版本、配方、SHA与许可证；对照相同中文提示及原生图片样本。OCR继续独立使用PP-OCRv5 mobile，不能把文本LiteRT试验记作OCR或多模态完成。
3. **实机GPU**：Windows Intel核显优先，补NVIDIA/AMD与Apple Silicon；Intel Mac、Linux分别核对。记录真实设备/驱动/后端、首句、解码、显存、进程树内存、与宠物游戏并行时的动画表现。当前没有`/dev/dri`设备，GPU未执行。
4. **产品回退**：包装进程上报实际后端，GPU初始化失败、内存不足、推理超时或崩溃由桌宠终止任务并回退已有本地CPU；不得自动改发云端。复用SQLite历史重建会话，避免依赖崩溃或取消后的KV状态。该回退仍是实施要求，本轮独立进程试验不能当作已接入产品。
5. **可选分发**：界面单独下载固定平台组件和兼容权重、展示实际大小与进度、SHA校验、可停止/删除；不在默认包同时预装两套完整权重。通过硬件和质量验收后再发功能tag。本次只提交研究文档和复现脚本，不发布新安装包。
