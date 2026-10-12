# Windows / macOS 系统代理

模型下载、可选加速组件下载及外接 HTTPS 服务启用 reqwest 0.12 的 `system-proxy`。Windows 读取当前用户 Internet Settings 中启用的代理；macOS 读取 System Configuration 中启用的 HTTP/HTTPS 代理。环境变量代理仍生效，明确设置的环境变量优先于相应系统代理配置。

每次新建联网客户端时读取配置；已经开始的下载不会中途切换代理。本地 CPU 和可选 GPU 的 HTTP 回环连接继续明确调用 `no_proxy()`，不经过系统代理。

支持系统的手动 HTTP/HTTPS 代理。此改动没有接入 PAC 脚本、WPAD 自动发现或 SOCKS 代理，也没有增加独立代理设置界面。macOS 的系统绕过列表不由此上游实现读取，可用 `NO_PROXY` 指定；Windows 上游实现读取 `ProxyOverride`，不代表支持所有系统例外语法。

验证：依赖锁定并核对 Windows x64 和 macOS ARM 的依赖树启用 `hyper-util/client-proxy-system` 及对应系统配置读取依赖；Linux `cargo check --locked --offline` 通过，`git diff --check` 通过。编译只报告既有 schemars cfg 和 `SayKind::Drag` 未使用警告。本环境只有 Linux，Windows/macOS 系统设置下的实际联网仍需实机验证。此改动尚未进入已发布的 alpha.6 安装包。
