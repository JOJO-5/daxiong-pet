大熊桌面宠物 v1.0.20：新增拔河，并修复球与飞盘松口时穿过胸前、前腿的问题。

- 拔河使用左右专属咬绳与撑地动作；拉住橙色绳环、轻拉再松手，可重复互动。
- 飞盘归还后，按钮从实际归还位置重新扔出。
- 球和飞盘从张开的嘴前松开，先避开前腿再落到脚边；松口动作与道具在同一画面更新。
- 松手后快速移开鼠标，也会从最后实际握住的位置抛出，避免玩具瞬移。
- 项目准则已要求新增动作逐帧核对原始角色、嘴部道具贴合与真实桌面录屏。

下载（Assets）：

| 系统 | 文件 |
| --- | --- |
| Windows x64 安装版 | `Daxiong-1.0.20-windows-x64-setup.exe` |
| Windows x64 便携版 | `Daxiong-1.0.20-windows-x64-portable.exe` |
| Mac Apple Silicon（M 系列） | `Daxiong-1.0.20-macos-arm64.dmg` |
| Mac Intel | `Daxiong-1.0.20-macos-x64.dmg` |
| Linux x64 AppImage | `Daxiong-1.0.20-linux-x64.AppImage` |
| Linux x64 Debian 包 | `Daxiong-1.0.20-linux-x64.deb` |

`SHA256SUMS` 包含全部安装包的 SHA-256 校验值。GitHub 自动附带的 Source code 是源码，运行程序请下载上表文件。

Windows 需要 WebView2 运行时。Mac 包未签名、未公证，首次运行按系统提示允许运行；全局鼠标查询需辅助功能权限。Linux 支持 X11 会话，需 WebKitGTK 4.1 与 AppIndicator；Wayland 原生会话暂未支持。
