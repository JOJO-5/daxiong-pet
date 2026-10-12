# alpha.7 系统代理、下载排版与原生输入验证

发布源码：`b4b5f1e6e8556bf27efd8a5e6dddee64c211d88f`，PR #9。版本为 `1.1.0-alpha.7`。[v1.1.0-alpha.7](https://github.com/JOJO-5/daxiong-pet/releases/tag/v1.1.0-alpha.7)已公开，保留完整与轻量两套下载和旧正式版。

## 变更与范围

Windows/macOS 启用 reqwest 的系统 HTTP/HTTPS 代理读取，下载模型、视觉组件、可选加速组件及外接 HTTPS 服务共用该能力。本地 CPU/GPU 回环推理继续直连。支持范围与限制见[系统代理记录](system-proxy-2026-10-12.md)；PAC/WPAD/SOCKS 未接入，Windows/macOS 用户网络下的实际下载仍需实机复核。

模型文件名与大小/状态分行，每项独立卡片。加速下载、试运行/关闭、删除操作分组，500px/380px 窗口均保留按钮间距，无水平溢出。完整包与轻量包均提供；light 保留 CPU 运行器、OCR 和许可证，用户点击下载聊天/视觉权重。GPU 加速仍为独立按需下载的实验性选项。

窗口重新显示时刷新原生点击穿透，仅在系统调用成功后缓存原生状态，前端事件缓存独立。Linux X11 直接读取已有 device_query 的全局物理光标，避免根窗口缩放参与坐标转换。菜单测试等待窗口可见和实际 GTK 弹出菜单映射后发送按键，并新增三轮隐藏/显示检查。

## 本地验证

- 前端测试15项、Rust124项通过，`git diff --check` 与菜单脚本语法检查通过。
- 普通 UID1000 的真实 Tauri/WebKit/X11 菜单回归26项、实际录屏22项通过，涵盖面板、飞盘、喂食、快捷键、重启、OS快捷键冲突、保存失败回滚、隐藏/显示后的鼠标命中。
- 真实下载设置500px/380px布局6项通过；实际点击新模型卡片开始网络下载与停止的3项检查通过，取消文件不成为已安装模型。
- 200% GDK 缩放下三轮隐藏/显示、真实鼠标悬停和菜单通过。首次独立探针误把 GTK 的1逻辑像素输入区固定断言成1物理像素；200%下实为2×2，改为对应物理尺度后通过。这是单屏缩放探针，不是混合DPI多屏验收。

本地证据位于 `/workspace/daxiong-ai-prototype/alpha7-menu-popup-ready`、`alpha7-menu-final-compat`、`alpha7-layout-final`、`alpha7-input-show-2x-corrected`、`alpha7-download-click`。记录对应真实原生应用，不采用 IPC 模拟或仅浏览器页面。

## 保留的失败与修复

第一候选 `bc49da0` 的 macOS ARM 完整包/启动检查通过，但 light DMG 脚本失败；增加详细日志和最多三次打包重试。第二候选 `8b87a42` 的 Mac ARM light 首次详细构建成功，没有用跳过步骤代替检查。

第一候选的完整桌面回归停在 light 下载脚本：界面已经改为 `.chat-model-card`，脚本仍查 `.chat-memory`，在点击前读取到不存在元素；修正真实界面选择器。

第二候选的完整桌面回归两次在录制菜单的首次悬停失败，原生输入区域保持1×1；未把失败当作通过。最终候选增加窗口显示时的原生输入刷新、Linux物理坐标读取与调试专用诊断。不能仅凭本地通过认定唯一根因；最终同源完整CI必须通过才发布。

本地菜单首次启动存在窗口尚未可见的脚本竞态；另一次边缘位置按键后等待面板超时。脚本增加窗口可见和 GTK 原生弹出菜单映射检查，而不是强制关闭产品穿透或绕过右键。最终本地兼容渲染和录屏两种路径通过。

## 最终 CI 与公开安装器

[四平台完整/轻量构建](https://github.com/JOJO-5/daxiong-pet/actions/runs/38264008834)和[同源完整桌面回归](https://github.com/JOJO-5/daxiong-pet/actions/runs/38264014475)均成功。最终两种Mac light DMG首次构建成功，没有触发重试。Windows/Linux Rust各124项、两种Mac各122项通过；原生交互Windows21项、两种Mac各15项通过，Windows球/飞盘实际运动4项通过。四个平台均检查实际light安装器：CPU运行器/OCR存在，聊天/视觉GGUF不存在。

完整Linux回归包括菜单26项/录屏22项、下载布局6项、CPU聊天24项、小输入框29项、屏幕观察/OCR23项、可选加速下载16项、损坏组件CPU回退6项及light界面真实模型下载/校验/CPU生成8项。生产版不带e2e时间加速，在64次实际办公点击期间141.33秒触发偶遇；这不是长期用户办公质量或性能结论。

公开资产为十个完整/轻量安装器与SHA256SUMS，tag指向上述最终源码；所有发布资产的GitHub digest与校验文件相符。实际下载Windows light EXE和Linux light deb，分别独立核对字节数与SHA-256；最新正式版仍为v1.0.27。

| 平台 | 轻量版字节数 | 完整版字节数 |
| --- | ---: | ---: |
| Windows x64 | 39,511,909 | 1,762,125,527 |
| macOS ARM64 | 61,133,098 | 1,850,759,877 |
| macOS x64 | 65,769,083 | 1,855,438,700 |
| Linux deb | 68,428,676 | 1,836,634,936 |
| Linux AppImage | 127,932,920 | 1,896,643,064 |

下载后的公开Linux light deb在普通UID1000下安装运行，12项通过：版本alpha.7、CPU/OCR存在且无权重、500/380px布局6项、真实模型下载接收字节/取消/残片清理3项。测试启动`/usr/bin/daxiong-pet`，不使用调试二进制或开发资源替代安装器；证据在`/workspace/daxiong-ai-prototype/public-alpha7-native`。公开下载证据在`/workspace/daxiong-ai-prototype/public-alpha7/download-check.json`；最终CI原始证据位于artifact11755178730（18,012,099字节），核对记录在`/workspace/daxiong-ai-prototype/alpha7-final-ci`。

原始角色素材和图集没有改动；本次不是姿态/嘴部视觉重新验收。真实Windows/macOS代理联网、消费级8GB/11代i5性能、GPU硬件速度/驱动、混合DPI与长期办公质量仍未验证。
