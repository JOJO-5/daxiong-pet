# alpha.8 手动视觉反馈交付

[v1.1.0-alpha.8](https://github.com/JOJO-5/daxiong-pet/releases/tag/v1.1.0-alpha.8)已公开，源码`e36f1f9bc54db398c7fd9dd50ca28881bade23b8`，PR #10。原始角色图集未修改。

## 变化

OCR完成后继续显示准备视觉模型、图片推理和完成的阶段；手动结果同步到宠物气泡与原聊天设置页。结果不写聊天记录或长期偏好，取消/暂停、切窗失效、最长120秒屏幕上下文规则保留。自动后台观察仍默认OCR；手动看图继续CPU。

保留完整版与light。完整版预装2B和配套视觉组件；light保留原生CPU运行器/OCR，用户在界面下载权重。0.8B文字模型不能替代2B视觉路径。

## 同源验证

本地真实原生应用兼容/正常渲染各31项通过。CI明确开启`E2E_AI_VISION=1`，通过原生鼠标点击“理解图片”，要求实际内置模型非空回复、推理/完成状态及宠物气泡中的同一回复。

[完整桌面回归38275288577](https://github.com/JOJO-5/daxiong-pet/actions/runs/38275288577)成功；观察/视觉27项、CPU聊天24项、小框29项、布局6项、light界面模型下载后聊天8项、加速下载16项/CPU恢复6项、菜单26项/录屏22项通过。生产版未启用e2e加速，在65次实际办公点击期间142.65秒触发偶遇。

[四平台完整/light构建38275284959](https://github.com/JOJO-5/daxiong-pet/actions/runs/38275284959)全部成功，含十个完整/light安装器、四平台原生CPU模型探针及平台原生启动/交互检查。Windows/Linux Rust各124项、两种Mac各122项通过；每个平台核对了实际light安装器的CPU运行器/OCR存在且不含聊天/视觉GGUF。

最终桌面CI证据artifact11760491764（17,941,032字节），已实际下载并读取`ai-observation/local-vision-result.json`与气泡截图。证据在`/workspace/daxiong-ai-prototype/alpha8-final-ci`。

## 明确保留的质量失败

上述CI的一次真实内置视觉回复说“虽无法读取屏幕内容”，尽管应用确实传入图片与OCR。这条回复的能力自述不正确、没有准确解释画面；不能把非空回复/显示成功写成语义验收通过。测试验证调用、状态与显示，不是50张独立图片或真实办公质量基准。原有视觉研究中的误读/注入失败继续保留，本轮没有宣称模型质量提升。

Windows用户的具体按钮、组件状态与原现象尚未取得实机反馈；本次修复已确认的反馈问题，不声称所有机器上的唯一原因已确定。真实硬件速度、多屏混合DPI和GPU硬件资格继续未验证。

## 公开安装包

公开资产包括十个完整/light安装器及SHA256SUMS，所有GitHub资产digest与校验文件一致；tag指向最终源码，最新正式版仍为v1.0.27。实际下载并独立核对Windows light EXE与Linux light deb的字节数和SHA-256。

公开Linux light在普通UID1000下安装运行，10项通过：版本/资源/没有开发覆盖，固定2B与视觉缓存的SHA校验，原生点击看图后的实际回复、处理阶段、同一结果的宠物气泡，以及应用数据中没有截图PNG。测试使用`/usr/bin/daxiong-pet`和安装器自己的运行器/OCR；权重复用已验证本地缓存，没有开发资源覆盖或冒充首次网络下载。合成样本回复“屏幕显示你正在整理笔记”，图像语义总体质量仍未通过独立数据集验收。

首次公开复核在启动应用前遇到跨设备硬链接EXDEV，是验证夹具失败；调整独立测试状态目录及缓存引用后10项通过，没有更换安装器或绕过模型SHA校验。证据在`/workspace/daxiong-ai-prototype/public-alpha8-native-final/e2e.json`与气泡截图；实际下载证据在`/workspace/daxiong-ai-prototype/public-alpha8/download-check.json`。


| 安装资产 | 字节数 |
| --- | ---: |
| Daxiong-1.1.0-alpha.8-light-linux-x64.AppImage | 127,937,016 |
| Daxiong-1.1.0-alpha.8-light-linux-x64.deb | 68,428,518 |
| Daxiong-1.1.0-alpha.8-light-macos-arm64.dmg | 61,131,824 |
| Daxiong-1.1.0-alpha.8-light-macos-x64.dmg | 65,766,490 |
| Daxiong-1.1.0-alpha.8-light-windows-x64-setup.exe | 39,509,558 |
| Daxiong-1.1.0-alpha.8-linux-x64.AppImage | 1,896,643,064 |
| Daxiong-1.1.0-alpha.8-linux-x64.deb | 1,836,635,138 |
| Daxiong-1.1.0-alpha.8-macos-arm64.dmg | 1,850,760,897 |
| Daxiong-1.1.0-alpha.8-macos-x64.dmg | 1,855,438,084 |
| Daxiong-1.1.0-alpha.8-windows-x64-setup.exe | 1,762,137,962 |
