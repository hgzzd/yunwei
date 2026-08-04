# 云尾兽（Yunweishou）

云尾兽是一只完全本地运行的 Windows 桌面宠物。它以透明、置顶、无边框的原生窗口贴在桌面工作区底部，按随机节奏活动；用户可以点击互动、拖到另一块屏幕，并通过右键/托盘菜单调整大小、声音、开机启动和显示状态。

首版目标平台是 **Windows 10/11 x64**。应用没有账号、后端服务或遥测；行为、教程与用户偏好只写入本机应用数据目录。默认关闭声音和开机启动。

## 技术栈总览

| 层级 | 技术 | 本项目中的职责 |
| --- | --- | --- |
| 桌面容器 | Tauri 2（Wry / 系统 WebView） | 创建原生窗口、托盘、菜单、事件与前后端命令通道；打包 Windows 安装器 |
| 原生业务层 | Rust 2021 | 行为状态机、窗口定位与移动、DPI/多显示器适配、设置持久化、全屏避让 |
| 桌面插件 | `tauri-plugin-autostart`、`tauri-plugin-single-instance` | 管理开机启动；保证再次启动时复用已运行实例并恢复显示 |
| 前端 | TypeScript 5 + Vite 6 | 应用入口、交互编排、构建开发服务器与生产静态资源 |
| 画面渲染 | 原生 Canvas 2D | 图集逐帧播放、左右翻转、高 DPI Canvas、透明边缘裁切和资源失败降级绘制 |
| 输入与声音 | Pointer Events、Web Audio | 区分点击与拖拽；播放本地 WAV 互动音效 |
| 资源 | 透明 PNG 图集、WAV、ICO/PNG/ICNS 图标 | 八类动作、两段音效与 Windows 应用图标 |
| 测试与质量 | Vitest 4 + V8 Coverage、Cargo Test、TypeScript | 前端纯逻辑单测、Rust 单测、类型检查、配置与资源完整性检查 |
| Windows 发布 | NSIS（Tauri Bundle）+ WebView2 bootstrapper | 生成当前用户范围的 x64 安装包；缺少 WebView2 时下载微软安装引导程序 |

当前锁定的主要版本可通过 `npm ls --depth=0` 与 `cargo tree` 复核：Tauri Rust 端 `2.11.5`、`@tauri-apps/api` `2.11.1`、CLI `2.11.4`、TypeScript `5.6.3`、Vite `6.4.3`、Vitest `4.1.10`。

## 架构

```text
                         ┌────────────────────────────────────┐
                         │         Rust / Tauri 主进程         │
                         │                                    │
                         │  行为状态机 · 设置存储 · 多显示器   │
                         │  拖拽定位 · 托盘菜单 · 全屏避让     │
                         └──────────────┬─────────────────────┘
                            invoke 命令 │ Tauri 事件
                                        │
              ┌─────────────────────────┴──────────────────────────┐
              │                                                    │
┌─────────────▼─────────────┐                      ┌───────────────▼─────────────┐
│ pet 透明宠物窗口           │                      │ bubble 透明气泡窗口          │
│ Canvas 图集渲染            │                      │ 教程与随机台词               │
│ 点击/拖拽/右键/键盘操作    │                      │ 不接收焦点、不拦截鼠标        │
└─────────────┬─────────────┘                      └─────────────────────────────┘
              │
              ▼
   public/assets/sprites + audio
```

### 运行时分工

- **Rust 主进程**：`src-tauri/src/lib.rs` 维护单一运行时状态。约每 33 ms 更新可见宠物的行为和位置；隐藏时降低检查频率。它通过 Tauri event 向两个前端窗口广播 `pet://motion-plan`、`pet://runtime-snapshot`、`pet://settings`、`pet://visibility` 与 Rust 下发的 `pet://tutorial-bubble-directive`。
- **行为状态机**：`src-tauri/src/behavior.rs` 在 15–45 秒间随机切换动作，在 3–8 分钟间随机显示一句台词。点击会触发摔跟头和固定气泡；拖动期间暂停随机状态切换。
- **窗口与屏幕适配**：宠物固定在当前显示器工作区底部，位置以“横向归一化坐标 + 显示器标识”保存。拖拽结束后会识别目标显示器、按该显示器 DPI 换算窗口尺寸并落底；显示器变更后自动回退到可用屏幕。
- **全屏避让**：Windows 上通过 `user32` 检查前台窗口是否覆盖其显示器；全屏时临时隐藏宠物与气泡，退出全屏后恢复。非 Windows 目标该检测为安全降级，不参与判断。
- **前端渲染**：`src/runtime.ts` 按窗口类型初始化宠物或气泡界面。`SpriteRenderer` 读取 JSON 图集清单，以 Canvas 绘制当前帧并根据朝向镜像；图片加载失败时绘制内置简化形象，避免白屏。
- **浏览器调试模式**：`npm run dev` 可预览前端动画和输入处理。桥接层会检测是否处于 Tauri 环境；在普通浏览器中不调用原生命令，因此不会有托盘、移动窗口、持久化或系统开机启动能力。

## 功能清单

### 桌面体验

- 透明、无边框、置顶且不出现在任务栏的宠物窗口；独立气泡窗口不会抢占焦点或阻挡鼠标。
- 八种状态：发呆、散步、奔跑、坐下、睡觉、伸懒腰、摔跟头、拖动。
- 左键点击互动；拖动可跨屏，松开后自动吸附到目标屏幕工作区底部；右键呼出上下文菜单。
- 支持键盘 `Enter`/空格触发点击、菜单键或 `Shift+F10` 打开菜单。
- 随机动作、行走/奔跑折返、随机台词与两段本地 WAV 音效。
- 检测前台全屏窗口时自动隐藏，避免遮挡游戏、演示或视频。

### 托盘与设置

托盘菜单提供显示/隐藏、120/180/260 px 三档尺寸、声音、开机启动、重置位置和退出。设置保存在应用数据目录的 `settings.json`，字段包括版本、尺寸、声音、开机启动、显示器、横向位置和首次使用教程进度；写入时先生成临时文件再替换，降低中断写入风险。

## 项目结构

```text
.
├── src/                         # TypeScript 前端与 Canvas 渲染
│   ├── runtime.ts                # 两类窗口的交互编排与事件订阅
│   ├── sprite-renderer.ts        # 图集加载、裁切与 Canvas 绘制
│   ├── gesture.ts                # 点击/拖拽手势判定
│   ├── pet-audio.ts              # 本地音效与节流
│   └── *.test.ts                 # 前端纯逻辑 Vitest 测试
├── public/assets/
│   ├── sprites/manifest.json     # 八种状态的图集和帧序列约定
│   └── audio/                    # chirp.wav、tumble.wav
├── src-tauri/
│   ├── src/lib.rs                # Tauri 生命周期、命令、窗口与运行循环
│   ├── src/behavior.rs           # 行为状态机
│   ├── src/model.rs              # 设置和事件载荷模型
│   ├── src/settings.rs           # 本地 JSON 设置存储
│   ├── src/platform.rs           # Windows 前台全屏检测
│   ├── tauri.conf.json           # 双窗口、CSP、NSIS 与 WebView2 配置
│   └── capabilities/             # Tauri 权限范围
├── scripts/                      # 配置与资源校验脚本
├── tools/                        # 图集、图标、音效处理辅助脚本
└── tasks/                        # 实施清单与复盘记录
```

## 环境要求

正式开发与发布请使用原生 Windows 10/11 x64：

- Node.js 20 或更高版本（建议当前 LTS）
- Rust stable，以及 `x86_64-pc-windows-msvc` target
- Visual Studio 2022 Build Tools：勾选“使用 C++ 的桌面开发”和 Windows 10/11 SDK
- Microsoft Edge WebView2 Runtime（Windows 11 和较新的 Windows 10 通常已预装）

环境安装细节以 [Tauri 官方前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 为准。

首次安装 JavaScript 依赖：

```powershell
npm ci
```

## 开发、测试与构建

### 常用命令

| 命令 | 作用 |
| --- | --- |
| `npm run dev` | 启动 Vite 浏览器调试页（端口 1420） |
| `npm run tauri:dev` | 启动完整 Tauri 桌面应用 |
| `npm run typecheck` | 仅执行 TypeScript 类型检查 |
| `npm test` | 运行 Vitest 前端测试 |
| `npm run test:coverage` | 运行前端测试并生成 `coverage/` 报告 |
| `npm run assets:check` | 校验图集、透明 PNG、WAV 文件和应用图标 |
| `npm run config:check` | 校验产品标识、双窗口契约和 Windows NSIS 配置 |
| `npm run build` | 类型检查后执行 Vite 生产构建，输出到 `dist/` |
| `npm run check` | 依次运行配置、资源、类型、测试和前端构建 |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 运行 Rust 行为、模型、存储和定位单元测试 |
| `npm run tauri:build:windows` | 构建 Windows x64 的 NSIS 安装包 |

提交前的最小验证集：

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

### Windows 安装包

在原生 Windows PowerShell 执行：

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:build:windows
```

NSIS 安装包输出目录：

```text
src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
```

安装器使用 `currentUser` 模式，安装在当前用户范围内，不要求管理员权限。若设备缺少 WebView2 Runtime，安装过程会联网下载微软 bootstrapper。当前项目未配置代码签名，下载或首次运行时可能出现 Windows SmartScreen 提示。安装与 WebView2 分发机制参见 [Tauri Windows Installer 文档](https://v2.tauri.app/distribute/windows-installer/)。

## 资源约定

`public/assets/sprites/manifest.json` 是图集的唯一运行时描述。每个状态必须声明帧索引、`fps` 和是否循环；当前统一按 `256 × 256` 单帧、`12 fps` 播放。`npm run assets:check` 会验证：

- `idle`、`walking`、`running`、`sitting`、`sleeping`、`stretching`、`tumbling`、`dragged` 八种状态齐全；
- 图集引用存在、文件为带 Alpha 通道的 PNG，且尺寸可被 256 整除；
- 帧索引未越界，WAV 文件为有效 RIFF/WAVE 格式；
- Windows 图标 PNG 与 ICO 文件有效。

替换美术或声音后，应先运行 `npm run assets:check`，再用 `npm run tauri:dev` 人工确认各状态的锚点、朝向和透明边缘。

## 隐私、网络与验收边界

应用运行时不主动请求网络，不依赖远端 API。唯一预期联网情形是安装器在系统缺少 WebView2 时下载微软 bootstrapper。

当前开发环境若为 WSL/Linux，可以完成前端检查和 Rust 单测；但它使用 Linux 图形栈，不能证明 Windows WebView2 的透明窗口、托盘、DPI、全屏检测或 NSIS 安装器真实可用。发布前必须在原生 Windows 10/11 x64 上完成以下人工验收：

- 安装与卸载、首次运行及托盘退出；
- 100% 与 150% DPI 的双显示器拖拽、任务栏贴底和显示器拔插；
- 睡眠唤醒、前台全屏隐藏/恢复；
- 至少两小时稳定运行。
