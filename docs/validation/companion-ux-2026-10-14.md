# 陪伴交互验收（2026-10-14）

候选alpha.10，尚待跨平台CI和安装包发布。改动说明见[体验目标](../DESIGN-PSYCHOLOGY-UX.md)。

## 已通过本地检查

- TypeScript/Vite构建通过，前端17项通过。
- Rust130项通过，包含暂时安静持久化/截止边界、严肃和开心回复动作、旧包和原始图集保留检查。
- 真实Linux X11/WebKit专项25项通过（/workspace/daxiong-ai-prototype/companion-ux-final/companion-ux.json），录屏和四组页面截图同目录。使用原生鼠标悬停、右键菜单和小框入口；实际等待125秒确认观察回复到期后不能回看、设置页也已清除。AI回复与屏幕事件为合成材料，不是模型质量测试。
- 已实际查看原始动作参考；没有图集/姿态/嘴部锚点改动。原生触摸等仍需本轮回归。
- 动作预览工具生成成功，奔跑/叼球/飞盘时长从原生轨道读取；生成文件/workspace/daxiong-ai-prototype/companion-ux-animation-preview.html。

## 保留失败与修正

- 首次桌面启动缺index.html：磁盘满后前端生成失败，原生构建读入不完整dist；清理可重建编译缓存后，先完成前端构建，再重建原生程序。失败证据companion-ux-native。
- 第二轮在WebDriver中等待原生菜单IPC返回导致超时；GTK菜单等待关闭是正常行为。改为真实右键与键盘选项，不把这次失败算通过。证据companion-ux-native2。
- 第三轮严肃回复断言时鼠标仍停在狗狗身上，触摸反馈优先；移开鼠标后复核安静动作，保持用户触摸优先。证据companion-ux-native3。
- 录屏发现系统title悬停提示遮住狗狗、设置页重复的启用提示占空间，已去掉系统悬停框并将启用提示限于聊天页；窄窗设置分类固定两列。第四轮23项通过，最后到期清理改动在final轮25项复核。

## 当前未验收

Windows/macOS本轮原生/安装检查、四平台构建和公开包待CI；Windows用户所见尾巴偶发外露、奔跑顿感继续未验证。CPU看图耗时、GPU硬件性能、小模型语义答偏保持原资格，不用UI测试代替。
