# 云尾兽

云尾兽是一只完全本地运行的 Windows 桌面宠物。它使用 Tauri 2、Rust、TypeScript 和 Canvas 构建，提供透明置顶宠物窗口、独立气泡窗口、随机行为、拖拽换屏、托盘菜单、全屏避让、开机启动和本地设置持久化。

首版面向 Windows 10/11 x64。应用不需要账号，不发送遥测，默认静音且不开机启动。

## 工程要求

- Node.js 20 或更高版本（建议使用当前 LTS）
- Rust stable，安装 `x86_64-pc-windows-msvc` 工具链
- Visual Studio 2022 Build Tools，勾选“使用 C++ 的桌面开发”和 Windows 10/11 SDK
- Microsoft Edge WebView2 Runtime；Windows 10 新版和 Windows 11 通常已预装

环境安装细节以 [Tauri 官方前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 为准。

首次安装依赖：

```powershell
npm ci
```

## 开发与检查

在 Windows PowerShell 中启动桌面应用：

```powershell
npm run tauri:dev
```

只启动浏览器前端调试页：

```powershell
npm run dev
```

提交前运行完整检查：

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run check` 会依次验证 Tauri/NSIS 配置、图集和音效文件、TypeScript 类型、Vitest 用例以及 Vite 生产构建。单独命令如下：

```powershell
npm run config:check
npm run assets:check
npm run typecheck
npm test
npm run test:coverage
npm run build
```

角色图集由 `public/assets/sprites/manifest.json` 描述，清单必须包含 `idle`、`walking`、`running`、`sitting`、`sleeping`、`stretching`、`tumbling`、`dragged` 八种运行时状态并引用同目录下带透明通道的 PNG。短音效放在 `public/assets/audio/*.wav`。`npm run assets:check` 会校验清单引用、资源文件头、透明通道和 Windows 图标。

## 构建 Windows 安装包

必须在原生 Windows 10/11 x64 的 PowerShell 中执行正式发布构建：

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:build:windows
```

NSIS 安装包生成在：

```text
src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
```

安装器使用 `currentUser` 模式，写入当前用户的 `%LOCALAPPDATA%`，不要求管理员权限。安装时若系统缺少 WebView2，会联网下载微软 bootstrapper。当前版本没有代码签名，浏览器下载或首次运行时可能出现 Windows SmartScreen 提示。

安装模式和 WebView2 分发方式见 [Tauri Windows Installer 文档](https://v2.tauri.app/distribute/windows-installer/)。

发布前需在 Windows 10、Windows 11 和 100%/150% DPI 双屏环境实际验证：安装/卸载、托盘退出、拖拽换屏、任务栏避让、显示器拔插、睡眠唤醒、全屏隐藏恢复，以及两小时稳定运行。上述人工验收不能由单元测试替代。

## WSL / Linux 说明

WSL 可用于前端检查。若还要编译 Tauri 并执行 Rust 单元测试，需先安装 Tauri 的 Linux 原生依赖：

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

然后执行：

```bash
npm ci
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

但 WSL 中运行的是 Linux 图形栈，不能证明 Windows WebView2 透明窗口、托盘、DPI、全屏检测或 NSIS 安装器可用。Tauri 虽提供基于 `cargo-xwin` 的跨编译路径，但它仍不能替代原生 Windows 的安装与交互验收；本项目的正式 `setup.exe` 必须在原生 Windows x64 构建并验证。

## 隐私与网络

云尾兽的行为状态、教程进度、音效和开机启动偏好仅保存在本机应用数据目录。应用本身不主动请求网络；联网只可能发生在首次安装且系统缺少 WebView2 Runtime 时。
