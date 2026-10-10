# 内置文字 AI 测试版验证（2026-10-11）

版本：1.1.0-alpha.1。固定 Qwen3.5 2B Q4_K_M 1,270,808,032字节及 llama.cpp d81235049384534c167caea52b85a694f6103d14，CPU静态运行器；安装包内包含模型、运行器、许可证和第三方通知。

Linux Ubuntu22.04/GCC11.4 生产 deb 已实际安装：`/usr/bin/daxiong-pet` 与 `/usr/lib/DaXiong/ai`。没有设置开发资源覆盖或外接服务，使用真实原生右键菜单打开聊天、点击开启、输入合成消息 `Hello, I had a tiring day.`，收到完整中文回复且SQLite状态为 complete。窗口截图实际查看，文字未被宠物遮挡；最小容器缺少emoji字体，爪印显示方框，不能代表普通桌面字体覆盖。关闭应用后没有残留 llama-server。

本地 deb 约1.18GiB，此本地包与CI最终下载资产分开记录；本地测试资源目录第三方通知后补，公开安装包由更新后的CI阶段生成。资源包未把模型写入Git；各平台先核对固定权重大小/SHA再打包。

证据：`/workspace/daxiong-ai-prototype/installed-ai-ui/{initial.png,reply.png,messages.json}`、`bundled-deb-install.log`、`bundled-deb-model.log`、`bundle-linux-static-package.log`。

此前失败：共享库版本具有编译目录RPATH，未作为公开AI包发布；改为静态链接，`readelf`验证没有工作目录RPATH，只依赖系统标准库。Windows模型成功生成中文，但Python cp1252 stdout打印发生UnicodeEncodeError；改为ASCII JSON日志。诊断工作流尝试读取中间日志未成功，已删除；首次构建在Intel Mac的缓存保存阶段取消以取得完整Windows日志，不算四平台整轮成功。

最终四平台构建与原生检查全部通过：[Build desktop packages](https://github.com/JOJO-5/daxiong-pet/actions/runs/38073266902)。安装包源码提交：`625b729ffbe656e205037d5826624dd604f52c79`。

复用的完整桌面回归：[Desktop E2E](https://github.com/JOJO-5/daxiong-pet/actions/runs/38071373049)，源码 `7a5164b076523c1d6b580412e59f9e7d87c2c2cc`。与发布提交相比，只差发布工作流、目标文件记录、model smoke脚本的ASCII日志和stage脚本第三方通知；`src/`、`src-tauri/`、`public/`、`assets/`、前端依赖/构建配置、固定模型清单与桌面测试脚本全部一致。发布工作流会核对差异，拒绝复用存在应用源码差异的回归。

预发布：[v1.1.0-alpha.1](https://github.com/JOJO-5/daxiong-pet/releases/tag/v1.1.0-alpha.1)，tag 指向上述安装包源码提交，保留现有正式版为 stable。

下载资产（字节）：

- `Daxiong-1.1.0-alpha.1-linux-x64.AppImage`：1345796600；`sha256:42907f402a30aa50c1ba31bea04fd9842f6c344c75224f85f62e8308ca066201`。
- `Daxiong-1.1.0-alpha.1-linux-x64.deb`：1267368062；`sha256:63bb81bbbd6872e3c2355dda2c950a630d4942c5f71e0f1c563f2ba8770be696`。
- `Daxiong-1.1.0-alpha.1-macos-arm64.dmg`：1281122847；`sha256:5f9c58f44b5c77c6c8c7eccda8e81f4728999080fde5002f6763740fad5ca818`。
- `Daxiong-1.1.0-alpha.1-macos-x64.dmg`：1281603746；`sha256:da33784e71b22b9be5af379446e4c13c474ad962566f5cf7a7ffd11e867d95c1`。
- `Daxiong-1.1.0-alpha.1-windows-x64-setup.exe`：1267597737；`sha256:c1704d7bc1c1b6f5023b1c6a94e773dcc99c237c41a35ca8046803776cc80cd2`。
- `SHA256SUMS`：525；`sha256:dbc66e3721d0066461e5c86c4f39e1401e86a157cbd1cbcf33d93d90d0edcf64`。

未验证：8GB/11代i5性能、Windows/macOS安装后的完整聊天GUI、目标设备帧率/RAM分布、真实2B五分钟空闲内存测量。此版只提供文字聊天、30天SQLite聊天记录和GUI性格；OCR、图像、观察、自动长期偏好及外接大模型未接入。
