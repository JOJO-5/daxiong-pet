# 独立 CPU 小模型原型

只测试语言模型，不接入宠物实时引擎、不读取真实屏幕、不修改用户数据库。模型和编译产物放在仓库外，不随本原型提交；分发前另行履行模型许可证。

## 复现（Linux x64 / AVX2）

需要 Python 3.9+、Git、CMake、GNU C++、make、约 3GB 以上可用磁盘及网络。此构建要求 AVX2/FMA/F16C，不是所有 CPU 的最低配置；Windows/macOS 需要各自原生构建和试验。没有 CMake 时可用隔离虚拟环境安装 `cmake`，不需要 PyTorch 或 GPU SDK。

从仓库根目录执行，使用一个全新的仓库外目录：

```bash
bash scripts/ai-prototype/build-runtime.sh /absolute/path/pet-ai-trial
python3 scripts/ai-prototype/download-models.py --models /absolute/path/pet-ai-trial/models
python3 scripts/ai-prototype/benchmark.py \
  --server /absolute/path/pet-ai-trial/runtime/build/bin/llama-server \
  --models /absolute/path/pet-ai-trial/models \
  --out /absolute/path/pet-ai-trial/results --threads 2 --limit 100
```

先用 `--limit 2` 和另一个输出目录检查模板/运行环境，再测完整样本。`--only QAD` 等参数可选择单一候选。输出目录必须为空，避免混入上次结果。服务器只监听本机 127.0.0.1:18891；需要该端口空闲。运行器固定 llama.cpp commit `d81235049384534c167caea52b85a694f6103d14`，模型 revision/字节/SHA-256 由[候选清单](../../docs/research/mobile-model-candidates-2026-10-10.json)固定，下载后及运行前均校验。

可另做短时空闲、断流取消、恢复、退出及 9 条诊断。先确保主 benchmark 已结束、端口空闲；模型已经完成哈希校验：

```bash
python3 scripts/ai-prototype/lifecycle.py \
  --server /absolute/path/pet-ai-trial/runtime/build/bin/llama-server \
  --model /absolute/path/pet-ai-trial/models/LFM2.5-350M-QAD-Q4_0.gguf \
  --out /absolute/path/pet-ai-trial/supplemental-LFM-QAD
```

`lifecycle.py` 内部调用 `diagnose.py`，后者只访问已运行的本机服务器。补测使用 temperature=0.1、min_p=0，与主测不同；只用于排查，不合并排名。空闲样本仅刚启动后 3 秒；取消通过关闭 SSE 传输实现，记录到 slot idle 的观察延迟，不是正式应用取消流程。

## 读结果

- `environment.json`：运行环境、CPU 配额、上下文和运行器版本。
- `*/responses.jsonl`：每条原始回复、首个非空正文耗时、总耗时、错误和严格检查结果。
- `*/summary.json`：新进程就绪、第一请求、其余请求 p50/p95、采样峰值 RSS/VmHWM。
- `*/server.log`：运行器加载、CPU 后端、prompt/decode timings。
- `*/unload.json`：服务进程关闭结果。

100 条是合成测试实例，动作识别包含 5 个命令各重复 3 次；不是 100 个独立唯一问题。50 条带严格 JSON/名称子串检查，另 50 条需要人工评审。严格偏好措辞检查可能把语义相同的改写判错，不能把通过率当作中文聊天质量。`make-cases.py` 可以重新生成固定用例；未使用真实用户聊天或截图。

上下文 2048、最多输出 96 token、并发 1、CPU 2 线程、GPU layers 0。Qwen 关闭 thinking。LFM/Qwen 的采样参数分别参考各自建议设置（本轮 min_p 统一采用运行器默认 0.05，Qwen 官方建议 0，后续再做参数对照），因此比较的是候选使用配置，不是仅量化位数的受控实验。每个请求关闭 prompt cache 复用。流式首正文并非完整第一句；总耗时可能包含输出长度差异。p95 采用排序下标 `floor((n-1)*0.95)`。

文件刚下载/校验，操作系统文件缓存是热的；新进程就绪时间不能宣称真正磁盘冷启动。内存为 Linux `/proc` 单个服务进程 RSS/VmHWM，不是私有内存或安装体积。主测不涵盖空闲 CPU/取消，可运行上述 lifecycle 补测；宠物游戏并行、完整包和真实 Windows/macOS 需单独验证。

[本次实测报告](../../docs/research/ai-cpu-trial-2026-10-10.md)。
