# 本地 AI 文字聊天：第一轮开发验收

开发与验收跨 2026-10-10–11，北京时间；本次收尾记录为 2026-10-11。

本次按用户“开始开发吧”实施 G2 的文字聊天部分：Qwen3.5 2B Q4_K_M、独立 CPU 进程、聊天窗口、性格 GUI 与 SQLite。没有改动角色图集、嘴部锚点或动作播放。版本仍为 1.0.27，新增代码是开发分支，尚未发布带模型的正式安装包。

## 实现范围

- 宠物右键菜单、托盘和互动面板均有聊天入口。首次需明确启用，普通宠物启动不会创建 AI 数据库或加载模型。
- 消息逐步显示，支持停止、关闭窗口取消、失败后重新编辑发送。收到的部分文字仍保留；旧请求的停止/关闭不能取消新请求。
- 性格界面可改狗狗名字、称呼、描述、语气和回复长短；恢复默认需要保存，试聊使用未保存的草稿、真实调用模型，不写入聊天记录。
- `ai.sqlite3` 与原 `companion.json` 分开，保存聊天、性格与设置；默认清理 30 天以前的聊天。迁移使用事务，遇到未来版本或损坏文件报错并保留原文件。
- 请求只取最近最多 12 组完整对话，实际套用模型模板并计算 token，在 4096 token 内预留输出与安全余量，优先移除最旧完整对话；当前消息过大则提示缩短。
- 模型固定为已确认的 2B 权重，首次加载校验长度及 SHA-256。服务绑定本机回环地址和随机认证密钥；推理、HTTP、SQLite 都离开宠物引擎与窗口事件线程。默认 2 线程，可选择 1–4 线程，空闲 5 分钟卸载。
- 聊天获得焦点时临时降低狗狗的置顶层级，离开或收起聊天后恢复。窗口事件缓存聊天的物理范围，宠物引擎只读该小范围缓存，阻止被聊天盖住的狗响应按钮点击；不在每帧查数据库、请求模型或查询聊天窗口位置。
- Linux 使用稳定的进程启动线程及父进程死亡信号：回复线程结束后继续复用模型，应用崩溃后清理模型。正常退出、取消和禁用均显式终止并回收子进程。Windows/macOS 的异常崩溃清理仍待原生验证和补齐。

偏好表只是后续 G3 的数据基础，**没有自动提取长期偏好**。这轮没有屏幕捕获、OCR、图像投影器、主动观察/发言调度、外接大模型设置或系统凭据存储。

## 开发复现

模型权重不提交 Git。开发者先准备清单中的固定权重与 `llama.cpp` 源码；用户最终安装包应内置资源，不需要这些开发步骤。

```bash
# 在目标系统/兼容工具链里编译；源码必须是清单固定的干净提交。
python3 scripts/build-ai-runtime.py --source /path/to/llama.cpp --build-dir /path/to/runtime-build

# 开发阶段可用符号链接，正式打包必须用复制模式。
python3 scripts/stage-ai-resources.py \
  --runtime-dir /path/to/runtime-build/bin \
  --model /path/to/Qwen3.5-2B-Q4_K_M.gguf \
  --runtime-license /path/to/llama.cpp/LICENSE \
  --out /path/to/chat-native-assets --symlink

DAXIONG_AI_RESOURCE_DIR=/path/to/chat-native-assets npm run tauri dev

npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol,e2e
cargo build --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol,e2e

# 真实 Tauri + WebKit + X11，需已有 Linux E2E 依赖。
DAXIONG_AI_RESOURCE_DIR=/path/to/chat-native-assets \
  E2E_SCRIPT=scripts/e2e-ai-chat.py E2E_RECORD=1 \
  xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash scripts/e2e-linux.sh
E2E_SCRIPT=scripts/e2e-ai-unavailable.py E2E_VISUAL=1 \
  xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash scripts/e2e-linux.sh
```

复制模式加 `--bundle-config /path/to/ai-resources.json` 可生成独立 Tauri 资源映射，目标为 `ai/runtime` 和 `ai/models`。这只是打包准备入口，尚未完成正式安装包、离线新装和跨平台验收。x86 构建要求 AVX2/FMA/F16C，不能把“关闭 GPU”说成适配所有 CPU。

## 本轮验证与限制

- 前端 15 项测试、生产构建通过。Rust 100 项测试通过，包括保留期边界、偏好来源在聊天删除后保留、未完成回复重启恢复、完整对话裁剪、断流、取消、旧请求隔离、模型启动线程寿命，以及真实子进程的空闲卸载/回收。
- 本地原生环境：Ubuntu 22.04 容器、GCC 11.4、GTK/WebKit/X11；与前期推理试验不同，容器未设置额外 CPU/内存配额。它仍是共享云端 Linux，不能作为 8GB/11 代 i5 或 Windows/macOS 性能结论。
- 旧宿主机模型运行器依赖 GLIBC 2.38、较新 libstdc++，无法在 Ubuntu 22.04 运行；已经改为在兼容工具链内重新编译同一个固定源码。原尝试及构建日志保留在 `/workspace/daxiong-ai-prototype/ai-dev-*`。
- 原生缺资源/损坏库测试、真实模型聊天与原有桌面互动回归已经通过。最终证据表在下面；模型期间捡球完成只验证行为能并行，没有把它当作 FPS/帧延迟性能验收。

| 场景 | 结果 | 本地完整证据目录 |
| --- | --- | --- |
| 真实 2B 模型聊天、捡球并行、停止、关闭、草稿试聊、崩溃重试、重启、清理及遮挡点击 | 23 项通过，退出码 0 | `/workspace/daxiong-evidence/local-linux/ai-chat-model-r5` |
| 缺资源、恢复默认/放弃草稿、窗口数据权限、损坏数据库 | 6 项通过，退出码 0 | `/workspace/daxiong-evidence/local-linux/ai-unavailable-r5` |
| 原有捡球、真实拖放、隐藏/专注/拖动取消、旧图集切换 | 12 项通过，退出码 0 | `/workspace/daxiong-evidence/local-linux/ai-chat-baseline-r3` |
| 原生右键菜单、飞盘、喂食、快捷键、磁盘失败回滚、自然邀请、练习与专注 | 23 项通过，退出码 0 | `/workspace/daxiong-evidence/local-linux/ai-chat-menu-r2` |

[检查明细与源码/二进制哈希](evidence/local-ai-chat-2026-10-11/checks.json)、[真实回复截图](evidence/local-ai-chat-2026-10-11/chat-reply.png)、[性格试聊截图](evidence/local-ai-chat-2026-10-11/personality.png)、[小窗口截图](evidence/local-ai-chat-2026-10-11/chat-small.png)随代码提交。完整原生录屏为模型证据目录中的 `desktop-validation.mp4`，哈希也在明细中；录屏留在工作区，没有把模型权重或视频放入 Git。

最后一次首次完整回复耗时 51.67 秒，包含**调试构建**的 1.27GB SHA-256 校验、加载、生成与同时录屏/桌面回归；只记录一次，不当作 p50/p95、真正冷盘启动或 8GB/11 代 i5 性能。取消会终止模型进程，下次回复需要重新加载。正式版的实际校验耗时、热回复延迟、长期内存和帧干扰仍需单独基准测试。

保留的失败：桌面回归第一次因测试只移动一次鼠标、没有跟随正在移动的宠物而超时；改为跟随实际窗口位置后原生回归通过。损坏库测试第一次留下有效 WAL，SQLite 正常恢复了故意破坏的主文件；清理隔离测试库的恢复日志后损坏路径通过。菜单测试第一次在窗口 ready 回调显示之前断言可见；等待实际可见后重验。这些失败不删除，也不拿后续成功代替未经验证的平台。

模型原生第一次完整测试到清空聊天时，狗狗挡住确认按钮，点击被狗截获；修正窗口层级后复验。截图又显示两个置顶窗口仍可能互相覆盖，因此进一步改成聊天获焦时把狗放入普通层，并屏蔽聊天范围内对背后宠物的点击。另一次测试在 native-ready 显示之前直接找 X11 窗口，现已等待实际可见。所有原始尝试分别留在 `ai-chat-model`、`ai-chat-model-r2`、`ai-chat-model-r3`、`ai-chat-model-r4`，不会覆盖。

聊天数据库保留 30 天，界面目前显示最新 200 条，模型上下文最多 12 组完整近期对话；这三个限制分别管理。历史分页、旧 companion 称呼与 AI 称呼的迁移/统一、外接后端及系统凭据仍需后续补齐。空闲 5 分钟释放已用真实轻量子进程模拟时间验证，本轮没有等待真实 2B 服务空闲 5 分钟并测进程树内存。异常数据库的 WebDriver 重启截图有过未重绘现象，功能检查基于真实 WebKit 错误 DOM 和文件未被覆盖；不把空白截图作为错误界面视觉验收。

G2 外接后端、G3–G6 与 G0 的发布收尾仍未完成。下一步补跨平台模型进程验证及 G2 外接后端，继续按已确认路线做明确偏好的自动记忆、单次 OCR/图像观察，再接独立观察/发言调度和完整 AI 安装包。
