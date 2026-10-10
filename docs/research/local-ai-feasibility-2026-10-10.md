# 大熊本地小模型可行性调研

调研日期：2026-10-10（北京时间）。范围：现有 v1.0.27 架构、官方模型卡、推理框架文档和量化发布者的文件元数据。本次没有下载模型或运行推理，文件体积是已查询事实，延迟、内存和中文交互质量尚未实测；本文件是设计建议，不代表已接入或已跨平台验证。

## 后续需求更新

2026-10-10：用户进一步要求默认包内置小模型、可外接大模型，并尽量降低体积。最新选型与打包方向以 [手机/端侧小模型补充调研](mobile-small-models-2026-10-10.md)和 [执行目标](../LOCAL-AI-GOALS.md)为准。本文件下文保留前期建议作为研究记录，其中“默认模型按需下载”“优先内置完整视觉模型”的旧方向不再作为默认交付要求。

## 结论

可以加入可选本地 AI。优先验证 Qwen3.5-2B 的短句互动，低资源档验证 Qwen3.5-0.8B；这两个模型本身支持语言和图像输入，不必默认同时加载一个语言模型和另一个视觉模型。中文密集小字另用 OCR，更容易控制识别范围与处理成本。

“鲜活”的主要收益来自互动记忆、说话时机、动作回应和一致性格，而不是每帧调用模型。模型负责提出短句与已有动作建议，Rust 状态机负责最终执行，宠物原始图集和角色形象继续遵守 AGENTS.md。

## 现有接入基础

- `src-tauri/src/engine.rs`：已有事件、活动指令、游戏优先级、专注/睡眠/拖动状态；实时行为必须继续独立运行。
- `src-tauri/src/companion.rs`：已有本地昵称、好感、摸摸/接球/喂食计数与训练进度。适合提供结构化上下文，不是现成的自由对话记忆。
- `src-tauri/src/main.rs`：已有 `pet:say`、`pet:message` 和引擎命令通道，可在旁路增加异步生成任务。
- `src/speech.ts`：目前是预设话术表与选择逻辑，可以保留为无模型、超时或不合格输出的回退。
- 尚无模型运行器、用户对话入口、截图授权与捕获、OCR 推理或模型下载管理。这些都是实际实现工作，不能仅换话术表就获得看屏幕能力。

## 候选模型与实际文件体积

以下 MB/GB 均使用十进制；不是安装包总大小，也不是运行内存。看图需要语言权重和匹配的视觉组件，不能只算语言文件。

| 候选 | 已核实的文件 | 用途与选择意见 |
| --- | --- | --- |
| Qwen3.5-0.8B，Q4_K_M | 527,502,816 字节语言权重 + 207,345,952 字节 BF16 视觉组件，合计约 0.735 GB | 低资源原型：简单短句、简单物体/画面描述。官方将此规模定位于原型和专用任务；不要据此保证稳定中文角色表现。 |
| Qwen3.5-2B，Q4_K_M | 1,270,808,032 字节语言权重 + 671,372,416 字节 BF16 视觉组件，合计约 1.942 GB | 优先评测的完整档：中文陪伴短句与单张图片理解共用模型。官方综合基准较 0.8B 更强，但大熊角色质量需独立评测。纯文字阶段可不加载视觉组件。 |
| SmolVLM2-500M，Q8_0 | 436,808,704 字节语言权重 + 108,785,184 字节 Q8 视觉组件，合计约 0.546 GB | 小体积视觉对照组。原始模型卡标注英语，不作为中文陪伴首选；模型卡中的视频显存数字不能套用到我们的量化桌面场景。 |
| Qwen3-1.7B | 本次未核定所选量化文件体积 | 纯语言对照/备用：官方明确支持本地推理与关闭思考。是否更适合中文短句，须和 3.5 的两档实测比较。 |

量化文件来自发布者 API 查询，不将社区转换当成 Qwen 官方原始权重；锁定 revision 和文件哈希后再分发。查询到的 revision：

- `lmstudio-community/Qwen3.5-0.8B-GGUF`：`26bab2c9369648924251c0ebb3dae012f5147707`
- `lmstudio-community/Qwen3.5-2B-GGUF`：`bb84e11355a036e28f080c7793fa6d22b7c4e344`
- `ggml-org/SmolVLM2-500M-Video-Instruct-GGUF`：`ccd7aae53bcb1997355c2f094959e72b3642ce17`

来源：[Qwen 0.8B 原始模型卡](https://huggingface.co/Qwen/Qwen3.5-0.8B)、[Qwen 2B 原始模型卡](https://huggingface.co/Qwen/Qwen3.5-2B)、[0.8B 量化文件](https://huggingface.co/lmstudio-community/Qwen3.5-0.8B-GGUF/tree/main)、[2B 量化文件](https://huggingface.co/lmstudio-community/Qwen3.5-2B-GGUF/tree/main)、[SmolVLM2 原始模型卡](https://huggingface.co/HuggingFaceTB/SmolVLM2-500M-Video-Instruct)、[SmolVLM2 量化文件](https://huggingface.co/ggml-org/SmolVLM2-500M-Video-Instruct-GGUF/tree/main)、[Qwen3-1.7B](https://huggingface.co/Qwen/Qwen3-1.7B)。

## OCR：识字和看懂画面要分开

OCR 返回文字、位置与置信度，不能独自判断“主人正在写报告”或理解图片内容。图像模型能描述场景，但小字、数字和密集界面仍应单独做识别验收。

跨平台首选评测 RapidOCR + ONNX Runtime，优先比较 PP-OCRv5 mobile 与最新 PP-OCRv6 tiny。Paddle 官方表列 v5 mobile 检测 4.7 MB、识别 16 MB；v6 tiny 检测 1.9 MB、识别 4.4 MB。这些是模型表中的体积，不含 ONNX Runtime、字典、转换后文件、预处理或运行内存；不能当成整个 OCR 模块只有 6.3 MB 或 20.7 MB。v6 对应 ONNX 文件、字典、算子与当前 RapidOCR 版本的组合还需验证，不能只换模型名字。

Mac 可评测 Apple Vision 的系统 OCR，减少额外模型下载，但中文支持应按系统版本运行时查询。Windows.Media.Ocr 官方要求桌面应用具有包身份；当前 NSIS 安装版与便携 EXE 不能假定符合该要求，因此不将它作为跨平台默认实现。Linux 继续走 OCR 模型。

来源：[Paddle 文字检测模型表](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/module_usage/text_detection.en.md)、[文字识别模型表](https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/module_usage/text_recognition.en.md)、[RapidOCR](https://github.com/RapidAI/RapidOCR)、[当前安装说明](https://rapidai.github.io/RapidOCRDocs/main/en/install_usage/rapidocr/install/)、[ONNX Runtime 平台与构建](https://onnxruntime.ai/docs/build/inferencing.html)、[Apple 文字识别](https://developer.apple.com/documentation/vision/vnrecognizetextrequest)、[Windows OCR 包身份要求](https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr?view=winrt-26100)。

## 怎么让宠物更鲜活

| 能力 | 具体体验 | 所需输入 |
| --- | --- | --- |
| 自然语言指令 | “大熊，趴一会儿”“再玩一轮飞盘”，回应一句并调用已有玩法 | 用户主动输入 + 当前允许的动作 |
| 有上下文的短句 | 远投飞盘回来后：“这次跑得好远！”；刚摸过头时语气更亲近 | 当前真实事件、玩法结果、已有好感 |
| 主人偏好记忆 | 用户说“晚上少说话”，保存可查看/修改的偏好；离线不惩罚 | 用户明确表达的偏好，不依靠模型虚构 |
| 看你给的图片 | 将猫或食物图片给它看，大熊歪头、嗅闻或说一句短评 | 拖入图片，或主动选择截图区域 |
| 看一眼陪你工作 | 点击“看看这里”，识别选区中的代码/文字，短句回应后继续安静陪伴 | 已授权的单次选区截图，必要时先 OCR |

先做自然语言互动与短句，再做主动图片入口，最后考虑可选的环境感知。拍屏幕不是鲜活的前提；自动环境感知若后续加入，应单独开关、低频处理、排除宠物自身窗口与敏感应用，不把每次 OCR 文本写入长期记忆。

## 运行和接入建议

推荐原型先连接已有 Ollama/LM Studio 本地服务，免去第一阶段的跨平台运行器打包；面向普通用户的版本，再提供应用管理的 llama.cpp 独立进程。接入本地服务不代表指定模型的图像协议和后端已经验证，必须实际发送图片测试。

llama.cpp 官方提供 CPU、Apple Silicon Metal、CUDA、Vulkan 等后端，是跨平台候选；Apple Silicon 优先 Metal，Windows/Linux 先验证 CPU 回退再验证 GPU，Intel Mac 不承诺等同 M 系列速度。单独的运行进程可隔离模型崩溃、方便停止和释放资源。预编译运行器需按四个平台固定版本、验证来源、签名/打包与完整依赖；不要为了演示把 Python/PyTorch 整套放进桌宠默认包。

建议数据流：用户输入/真实宠物事件 → 受控上下文 → 异步模型任务 → 验证短句及动作建议 → 现有 Rust 命令和气泡。截图入口先处理选区，再送 OCR 或图像模型；模型从不接管 60Hz 状态机。

- 文字模式初始上下文控制在 2K–4K tokens，输出短句，默认不开思考，不沿用模型卡里的超大上下文示例。
- 动作仅从已有允许列表选择，由状态机检查睡眠、专注、拖动、玩具状态；模型不能自行生成动画帧、直接改窗口位置或执行系统命令。
- 一次只运行一个生成任务；事件被打断、宠物切换或用户关闭功能时取消请求，丢弃过时回应。
- 高优先级交互立即用现有动作回应；模型慢或离线时使用已有话术，不让狗等推理才接球。
- 默认关闭 AI、模型按需下载；对话和看屏幕分开开关，本地处理，默认不保存原始截图。屏幕/OCR 内容是待理解材料，不是可执行指令。
- 模型开启期间设置可调整的空闲卸载；已有 Ollama 服务可使用 keep_alive，应用托管运行器则自行管理生命周期。频繁卸载会增加下一次启动延迟，应测量后定默认值。

来源：[llama.cpp 平台后端](https://github.com/ggml-org/llama.cpp)、[构建说明](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)、[多模态组件](https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md)、[Ollama 生命周期](https://docs.ollama.com/faq)。候选模型卡标注 Apache-2.0，运行器标注 MIT；实际发布仍应随所选版本保留许可证和通知，逐项核对模型、转换文件和运行时。

## 分阶段与验收

### CPU 优先与统一 GPU 后端（补充调研）

CPU 可以运行上述语言/图像模型：llama.cpp 提供 CPU 推理与 x86/ARM 优化。建议第一版以 0.8B 量化模型的纯 CPU 短句为基线，2B 为质量对照，不以“能加载”代替“能流畅陪伴”。看图同样可走 CPU，但图像编码与提示处理需要单独测量；初期只做主动单张图片或选区，不持续分析屏幕。OCR 使用 ONNX Runtime CPU 路径独立评测。

GPU 不必由项目分别实现各厂商算法：Windows/Linux 可优先采用 llama.cpp 的 Vulkan 后端，覆盖具有兼容驱动和所需特性的 NVIDIA/AMD/Intel 设备；Apple Silicon 采用 Metal。同一个模型文件可共用，应用调用接口保持一致，运行器仍需按系统/架构打包。统一接口不能免去显卡驱动、模型算子、显存和性能兼容性测试，集显加速也不保证比 CPU 更快。

Windows 的 ONNX 模型还有 DirectML（DirectX 12）跨厂商 GPU 路径；官方当前说明 DirectML 进入持续维护，新功能转向 WinML 的执行后端选择机制。此路线可用于 OCR/ONNX 模型，不能直接据此认定 GGUF/Qwen3.5 的整套推理都支持。第一版无须为 OCR 增加 GPU 依赖。

建议产品设置为“CPU（兼容）/自动加速”两档：默认 CPU，后续自动档尝试已测试的 Metal/Vulkan，失败则重启运行器回退 CPU；限制生成线程数、上下文和输出长度，模型任务与动画线程分离。纯 CPU 图像任务需同时关闭语言层和视觉组件的 GPU offload，而非只调整语言层参数。验证重点包括老 x86 指令集兼容、线程限制下的延迟、看图首句耗时和正常办公期间的响应。

来源：[CPU 构建与 Vulkan/Metal 后端](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)、[多模态 GPU offload 控制](https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md)、[ONNX Runtime 后端](https://onnxruntime.ai/docs/execution-providers/)、[DirectML 支持范围与 WinML 迁移说明](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)。尚未实际跑 CPU/GPU 模型基准。

1. 文字原型：固定大熊性格、中文短句、已有指令、事件上下文；比较 0.8B / 2B / 1.7B。先验证是否自然、有趣、少重复，不能只测能否输出文字。
2. 图片与 OCR 原型：主动图片/选区入口，分别测试普通物体、中文密集字、代码、混合中英文和低置信度。OCR 不确定时不编造，图片识别不确定时允许不发评论。
3. 可选本地模型包：四平台运行器、下载/取消/校验/删除、空闲卸载、离线回退、旧宠物兼容；上线前复查动作与原始角色一致性。

测试硬件至少包含 8GB Windows 集显机、16GB Windows、8/16GB Apple Silicon、Intel Mac 和 Linux CPU。以上是测试分组，不是保证可用的最低配置。

记录冷启动、热启动首句延迟、整句耗时、进程树峰值内存/显存、推理期间拖动与接球流畅度、空闲 CPU、卸载后释放、中文错误和错误动作率。至少准备 100 条真实宠物短对话/指令和 50 张屏幕/图片样本，人工评估性格与画面事实；先固定输入分辨率和上下文，再比较结果。

建议第一轮体验目标（不是现有成绩）：热启动短句约 2 秒内可见，5 秒超时回退，拖动/接球不受生成任务阻塞；达不到时优先减小上下文和模型档位，并保留普通桌宠模式。未测出上述数据前，不宣传低内存、秒回或全平台看屏幕已完成。

## 已确认的产品约束（讨论阶段）

- 聊天与主动陪伴都支持；观察频率、主动说话频率分别设置。
- 默认观察当前正在操作的窗口，允许切换为整个桌面。
- 性格通过界面编辑，不要求用户维护配置文件。
- 数据保存在本地 SQLite：聊天、明确表达的长期偏好、性格设置分别管理。
- 用户明确表达的偏好自动记住；屏幕内容仅作为短期上下文，不自动转为长期个人事实。
- 聊天记录默认保存一个月；保存期限不等于模型上下文长度。
- 每次推理仅组合有限的近期对话、相关长期偏好和仍然有效的屏幕摘要，按实际模型 token 预算裁剪。原始截图无需长期存储。
- 界面应提供查看、更正、删除已记住偏好的入口。
- CPU 延迟、中文聊天质量、视觉误读、上下文预算需要真实机器验证。以上为已确认方向，尚未实施 AI 功能。
