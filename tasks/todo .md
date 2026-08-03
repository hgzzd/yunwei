# 云尾兽 MVP 实施清单

- [x] 建立 Tauri 2 + TypeScript 工程与质量检查脚本
- [x] 实现 Rust 状态机、设置存储、显示器定位与全屏判定
- [x] 实现透明宠物窗口、气泡窗口、托盘菜单和开机启动
- [x] 实现 Canvas 渲染、点击、拖动、右键和首次引导
- [x] 生成并整理云尾兽正式位图、图标与原创音效
- [x] 补齐 Rust、TypeScript 与资源校验测试
- [x] 生成 Windows NSIS 配置并完成可执行范围内的构建验证
- [x] 检查差异、运行完整测试并记录结果

## Review

### 已交付

- Tauri 2 + Rust + TypeScript/Canvas 桌面应用，包含透明置顶宠物窗口、独立气泡窗口、托盘菜单、单实例和本地设置持久化。
- 八种行为状态：发呆、散步、奔跑、坐下、睡觉、伸懒腰、摔跟头和拖动；支持点击反馈、跨屏拖放、DPI 缩放、任务栏工作区贴底及前台全屏自动隐藏。
- 默认关闭音效和开机启动；提供显示/隐藏、三档尺寸、音效、开机启动、位置重置和退出菜单。
- 八套透明 PNG 动画图集、Windows 图标、两段原创 PCM WAV 音效，以及可复现的资源处理和校验脚本。
- Windows 10/11 x64 的 NSIS `currentUser` 安装配置，并配置缺少 WebView2 时使用微软 bootstrapper。

### 自动验证结果

- `npm run check`：配置、资源、TypeScript 类型、18 个 Vitest 用例和 Vite 生产构建全部通过。
- `npm run test:coverage`：5 个测试文件、18 个用例通过；行覆盖率 76.03%，分支覆盖率 77.63%。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`：12 个 Rust 用例通过。
- Linux 原生目标：`cargo fmt --check` 和 `cargo clippy --all-targets -- -D warnings` 通过。
- Windows `x86_64-pc-windows-gnu`：`cargo check` 和严格 `clippy` 通过。
- Windows `x86_64-pc-windows-msvc`：`cargo check` 和严格 `clippy` 通过；WSL 主机使用 LLVM 资源编译器完成静态检查。
- 八套最终图集均为 `1024x256 RGBA`，资源校验通过，透明区域与绿幕残留检查通过；两段音效均为 48 kHz、16-bit、单声道 PCM。
- 源文件尾随空白检查通过；`dist`、`node_modules`、Tauri `target/gen` 和临时审图文件均已忽略。

### 环境限制与 Windows 验收

- 当前环境为 WSL2。Tauri Linux 可执行文件能够完成编译并进入启动阶段，但 WSLg 的 `tao` 事件循环在本机图形设备权限/后端组合下崩溃；软件渲染重试结果相同，未修改 Windows 实现来规避该环境问题。
- WSL 无法生成和验证正式 MSVC/NSIS `setup.exe`，也不能证明 WebView2 透明窗口、托盘、Windows DPI、全屏检测等真实交互有效。
- 发布前必须按 `README.md` 在原生 Windows 10/11 x64 构建安装包，并完成安装/卸载、100%/150% 双屏、显示器拔插、睡眠唤醒、全屏隐藏恢复和两小时稳定运行验收。
