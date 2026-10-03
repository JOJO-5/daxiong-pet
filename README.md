# 大熊桌面宠物

一只住在桌面上的宠物。它会盯着你的鼠标看、没人理时自己溜达、被摸头会抬爪回应、
被连点会翻脸、拖起来松手会带着惯性滑出去，开着重力还会掉到屏幕底部弹一下。

基于 **Tauri 2 + React 18 + TypeScript**。Rust 侧负责全部行为逻辑与窗口物理，
前端只做精灵图渲染。绿色版单文件约 6.5 MB，内存占用低。

## 功能

### 互动玩法

| 功能 | 触发方式 |
| --- | --- |
| **附近快捷菜单与可选快捷键** | 右键大熊可直接喂饼干、扔球、扔飞盘；打开面板默认在大熊旁，记忆与开关收进「更多」；可启用 Ctrl+Alt+P，Esc 收起面板 |
| **偶遇小事件（可关闭）** | 空闲时蝴蝶来访、推球邀请；专注、睡眠、隐藏和手动互动时不打扰 |
| **零食与陪伴记忆** | 喂食有接住、咬下、咀嚼和舔嘴动作，奖励指令和找到零食也会吃；互动面板设置昵称，记录摸头与接球；熟悉后出现专属称呼，离线不扣分 |
| **扔飞盘** | 选择「飞盘」直接扔出或拖动甩出；浅弧线滑翔、跃起接盘、叼回放到脚边，再抓起重扔 |
| **藏零食找一找** | 互动面板选择「找零食」，拖动饼干放好后开始寻找；可选明显或稍远位置 |
| **小指令学习** | 「小指令」里选择过来、转圈、趴下、等一下；完成后奖励饼干，三次学会，重启保留进度 |
| **接球新玩法** | 近投慢捡、远投兴奋追；连续三次有不同庆祝，可推回归还的球；偶尔叼球邀你追，可关闭 |
| **扔球接回** | 右键大熊选择「打开互动面板」或从托盘打开「和大熊一起玩」，抛一球或拖动桌面球松手，大熊追上后叼回 |
| **鼠标跟随注视** | 用满 16 个方位帧，鼠标移到哪它就看哪 |
| **摸头** | 光标在它身上停住约 1.2 秒，会抬爪回应；生气时可安抚它 |
| **连点会生气** | 2 秒内连点 3 次就翻脸，之后几秒不搭理你 |
| **拖拽 + 惯性** | 拖动跟手，松手按甩出速度滑行、撞到屏幕边缘会反弹 |
| **重力开关** | 开启后松开手往下掉，落到屏幕底部会弹一下 |
| **自主漫游** | 没人理的时候自己起身溜达，撞边掉头 |
| **会睡觉** | 3 分钟没人搭理就趴下睡着（压暗 + 呼吸起伏），鼠标一碰就醒 |

### 说话气泡

点击 / 拖拽 / 摸头 / 生气 / 睡着 / 醒来 / 溜达时都会冒出一句气泡。
台词可以在 `src/speech.ts` 里改，宠物包也能自带 `speech.json` 覆盖。

### 实用提醒

- **番茄钟** —— 托盘启动 25 分钟专注，期间它安静陪伴，偶尔变换姿态，时间到了跳起来提醒你休息
- **喝水提醒** —— 每小时主动提醒你起身喝水
- **整点报时** —— 每到整点报一下时间

### 其它

- 系统托盘：切换宠物、开关重力、开机自启
- **点击穿透**：鼠标不在宠物身上时窗口完全穿透，绝不挡桌面操作
- 透明无边框置顶窗口

## 宠物包

程序启动时自动扫描两个位置，任何符合 [codex 宠物 v2 契约](https://github.com/openai/codex) 的宠物包都能直接用：

- `${CODEX_HOME:-~/.codex}/pets/` —— 与 codex 本身的宠物目录共用
- `<exe 所在目录>/pets/` —— 绿色版便携位置

```
pets/
└── 任意名字/
    ├── pet.json          # 可选，没有就用目录名当宠物名
    └── spritesheet.webp  # 必需（.png 也认）
```

`pet.json`：

```json
{
  "id": "daxiong",
  "displayName": "大熊",
  "description": "一句话描述",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp",
  "sleepFrame": { "row": 5, "col": 2 }
}
```

- `spriteVersionNumber` 必须是 `2`
- `sleepFrame` 是本项目**可选的扩展字段**：契约本身没有睡觉动画，
  默认借 `failed` 行第 2 格（正好是趴着侧躺的姿态），不同宠物趴下的位置不同时可以覆盖
- 宠物包目录里还可放一个可选的 `speech.json` 定制专属台词：
  ```json
  { "click": ["..."], "drag": ["..."], "idle": ["..."], "wander": ["..."] }
  ```

**图集要求**：宽必须 **1536**（8 列 × 192），高必须是 **208 的整数倍**且至少 **9 行**。
程序读图片实际尺寸来校验，不看文件名。11 行是完整版（含注视），9 行会自动关闭注视功能。

新增宠物后，托盘右键 →「重新扫描宠物」即可看到。

## 构建

需要 Rust 1.88+（建议使用最新 stable）和 Node 22+。

桌面平台：

- Windows：WebView2 运行时，生成 NSIS 安装包和便携 exe。
- macOS：生成 Intel / Apple Silicon 的 DMG；全局鼠标按键查询需要在系统设置中授予辅助功能权限。未签名、未公证，首次打开需按 macOS 提示允许运行。
- Linux：支持 X11 会话，生成 AppImage / Debian 包；需要 WebKitGTK 4.1、AppIndicator 和 X11。原生 Wayland 的全局鼠标查询、窗口移动和点击穿透尚未支持，请选择 X11 会话。

GitHub Actions 的 `Build desktop packages` 自动构建以上平台，成功产物可在运行页面的 Artifacts 下载。不同平台不能在当前环境里完成桌面实测。

Android / iOS 暂未适配：Tauri 的移动应用支持不等于桌面悬浮宠物支持。Android 悬浮桌宠需要额外原生服务和权限，iOS 不提供同等的跨应用常驻覆盖能力。

```bash
npm ci
npm run tauri build -- --bundles nsis
```

产物：

- `src-tauri/target/release/daxiong-pet.exe` —— 绿色版，双击即用
- `src-tauri/target/release/bundle/nsis/*-setup.exe` —— 安装包

### 上游兼容处理

1. **`schemars` 报 E0107（3 个泛型参数）**
   `tauri-build 2.7` 硬编码启用了 schemars 的 `preserve_order`，而 schemars 0.8.22 的该 feature
   指向 `indexmap 1.x`、代码里用的却是 2.x 的泛型写法（`IndexMap<K, V>`），必然编译失败。
   这是上游已知问题（[schemars#261](https://github.com/GREsau/schemars/issues/261)），0.8 分支未修。
   本仓库的 `src-tauri/vendor/schemars` 是一份**只把 indexmap 依赖从 1.2 提到 2.0** 的官方 0.8.22 副本，
   通过 `[patch.crates-io]` 挂上，源码其余部分未改动。

2. **cargo 联网时长时间卡住**
   如果环境里注入了失效的代理（cargo 的环境变量优先级高于 `~/.cargo/config.toml`），
   或者 cargo 的全局缓存自动 GC 恰好扫到加密盘，启动阶段会卡很久。可以这样绕过：

   ```bash
   env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY \
     CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo build
   ```

3. **Linux 快捷键关闭后仍占用按键**
   `global-hotkey 0.8.0` 的 X11 注销请求未确认发送。`vendor/global-hotkey` 仅将该注销循环改为确认每次系统响应后再返回，其余平台源码保持官方版本；许可证与修补说明保留。原生 E2E 通过第二个进程实际抢占按键验证释放、冲突和恢复。

## 项目结构

```
src/                      前端（纯渲染，不含行为逻辑）
  App.tsx                 精灵图渲染 + 气泡
  speech.ts               话术表
  styles.css
src-tauri/src/
  main.rs                 窗口、事件、命令、引擎线程
  engine.rs               行为引擎（状态机 + 物理 + 计时）
  petpack.rs              宠物包发现与契约校验
  tray.rs                 托盘菜单
  platform.rs             Win32 调用（光标 / 按键 / 本地时间）
  config.rs               配置持久化
  atlas.rs                精灵图契约（行定义与播放时长）
```

### 设计要点

- **Rust 是唯一真相源**：行为状态机、计时、帧播放、命中判定、窗口物理全在 Rust；
  前端只接收 `(row, col)` 画出来，无处可变状态之外的职责。
- **点击穿透下鼠标事件到不了 WebView**，所以按键状态改用 Win32 `GetAsyncKeyState` 全局读取，
  拖拽 / 点击全在 Rust 的 16ms tick 里判定（位移 > 6px 算拖拽，否则松手算点击）。
- **命中热区以「宠物矩形」为基准**而非整个窗口（窗口上方还留着气泡的空间），
  所以放大窗口不会挡住桌面点击。

## License

MIT


macOS 构建：`npm run tauri build -- --bundles dmg`。
Linux 构建：`npm run tauri build -- --bundles appimage,deb`。
行为回归测试：`cargo test --locked --manifest-path src-tauri/Cargo.toml engine::tests`。


## 1.0.1 更新

两轮可靠性优化：重新扫描立即更新当前图集、话术和睡眠帧；图片解码失败会恢复内置大熊；睡眠帧按实际图集校验；番茄钟使用完整经过时间，物理运动单独限制步长。

内置大熊始终可选。重复启动会显示已有窗口；隐藏时暂停鼠标查询和窗口运动，轮询降为每秒 2 次，睡眠时每秒 10 次，醒来恢复约 60 次。提醒计时继续运行。配置原子写入，保存、自启等操作失败时显示原生错误提示。

GitHub Actions 运行前端图片测试、Rust 回归测试，以及 Linux X11 环境下的启动 / 重复启动检查。Windows、macOS 的桌面交互与实际功耗仍需实机验证。

## 趣味性升级（1.0.2 / 1.0.3）

- **1.0.2：互动反馈**。连续触发同一动作也会重新播放；轻抚能安抚怒气，摸头和拖拽计入互动；区分轻放与快速甩动，甩动后的首次碰撞有落地回应。停止移动后再松手按轻放处理，速度阈值按屏幕缩放归一。
- **1.0.3：性格与自主行为**。内置大熊使用边牧专属台词，按类别避免近期重复；加入低频抬爪、歪头和打量动作，专注期间保持安静并偶尔变换姿态。

新增话术类别 `gentle_drag`、`throw`、`land`、`comfort`；旧宠物包的 `drag` 和 `pat` 自定义台词会自动作为回退。

## 1.0.4 专属动画

现有 11 行全部进入行为逻辑：思考动作加入安静的自主行为。内置大熊追加开心摸头、睡眠呼吸、醒来伸懒腰三行（8 帧/行），分别对应两组新互动。原有前 11 行与外部宠物包布局保持兼容。

美术参考原大熊图集，经图像生成后整理为统一的 192×208 帧，来源保存在 `assets/animation-source/`。1.0.4 使用三行扩展图集；当前版本请使用下方五行重建命令。

## 1.0.5 专属动画

追加甩毛恢复、歪头抬爪撒娇两行。快速甩动后等速度降低、重力模式下落地停稳再甩毛；撒娇会出现在点击回应或低频自主动作中，专注时不主动撒娇。

内置大熊最终使用 16 行图集（原 11 行 + 新 5 行，共四组动作），完整扩展图集为 1536×3328。外部 9 / 11 行宠物继续使用原有动作，切换宠物会取消尚未结束的扩展动作，避免空白帧。运行 `bash scripts/pack-animations.sh 5` 可重建完整图集；生成原图与分帧坐标均保留在仓库中。

## 1.0.6 大熊形象一致性修正

重做新增四组动作：使用原版站立、趴卧、伸展与抬爪帧作为生图参考，恢复修长身材、长腿、原版嘴鼻和像素颗粒。打包时每组使用固定比例与最近邻采样，避免柔化像素边缘；保留原有前 11 行的可见像素与行为触发规则。原参考图、生成稿和分帧坐标均保存在 `assets/animation-source/`，完整图集仍为 16 行。

## 桌面端到端验证

[趣味互动路线](ROADMAP.md)分版开发，真实 Tauri + WebKitWebDriver + X11 鼠标 E2E 通过后合并。
Linux 测试需安装 WebKitGTK 4.1、`webkit2gtk-driver`、Xvfb、Openbox、Picom、xdotool、ImageMagick 和 D-Bus。

```bash
npm ci && npm test && npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo build --locked --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol,e2e
cargo install tauri-driver --version 2.1.0 --locked
E2E_MEMORY=1 E2E_ENCOUNTERS=1 xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash scripts/e2e-linux.sh
```

`e2e` 仅在调试构建中把事件等待间隔缩短到 3–4 秒，方便等待真实调度；正式构建仍为 90–150 秒，睡眠和专注时长不缩短。

Linux 桌面或装齐依赖的 Docker 容器都能本地运行上述回归，不必等待 GitHub Actions；没有系统安装权限时先检查 Docker 是否可用。
E2E 不模拟 IPC；检查实际窗口、按钮、鼠标抛球和 Rust 接回状态。结果、桌面截图与原生驱动日志输出到 `test-results/`。
回归客户端共用 `scripts/webdriver_http.py` 的持久 HTTP/1.1 连接，避免 `urllib` 强制关闭连接与 WebKit 响应缺少关闭标记造成的驱动连接复用竞态；鼠标、按钮和状态修改请求不自动重试。
Windows / macOS 的原生鼠标与多屏行为仍需对应平台实测，Linux E2E 不代表它们已实测。

### 视频验收

安装 `ffmpeg` 后，在上述 E2E 调试构建上运行：

```bash
E2E_RECORD=1 E2E_SCRIPT=scripts/e2e-video.py xvfb-run -a -s '-screen 0 1280x800x24' dbus-run-session -- bash scripts/e2e-linux.sh
```

输出 `test-results/desktop-validation.mp4` 和 `video-scenes.json`，记录接球、昵称与喂食、自然偶遇、自然睡眠和唤醒。调试构建只缩短偶遇等待，正式版仍为 90–150 秒。

状态回归使用 WebKitWebDriver 兼容渲染路径；该路径的透明窗口截图可能残留旧帧，不作为视觉通过证据。录屏使用默认 WebKit 渲染和 Picom 完整重绘。GitHub E2E 工作流将视频随验收结果上传。

1.0.10 修复叼球返回：内置图集追加左右闭嘴跑步两行，球按每帧嘴部锚点移动。原有 16 行像素保留；旧宠物包使用原跑步动作降级。可用 `E2E_RECORD=1 E2E_SCRIPT=scripts/e2e-carry.py` 按上面的 Xvfb 命令录制双向叼回验收。

1.0.11 将球画进左右咬球动作帧，上下颚夹住球，叼回时隐藏独立球窗口。新增左右松口动作：球连续落到脚边并保留，可以直接抓起再扔；准备与归还的球不会因 30 秒计时消失，主动结束玩球、隐藏或切换宠物时收起。内置图集为 20 行（1536×4160），原有前 16 行像素不变；外部旧图集继续降级。

## 1.0.19 拔河（待原生验收）

右键大熊选择「一起拔河」，或在面板里选择拔河、拿出绳子。按住桌面的橙色绳环，向外轻拉约两秒，再松手休息；大熊会反复使劲并开心回应。可以连续玩，不设输赢。原有 23 行保持不变，扩展两行原版形象的闭嘴咬绳撑地动作，逐帧对齐绳端；当前图集 1536×5200，共 25 行。

取消、切换玩法、隐藏和专注会清理绳子，拔河不增加接球次数。Windows 原生鼠标烟测、macOS 原生鼠标检查、Linux 原生录屏在 CI 上分别验证；结果与未覆盖的实机项目见 [跨平台验收说明](docs/PLATFORM-VALIDATION.md)。

角色一致性制作与验收遵循 [项目准则](AGENTS.md)：每次修改动作都要对照原始形象，并保留原生录屏。飞盘归还后可以抓起再扔，或点击「扔飞盘」从原归还位置继续投掷。
