# 云尾兽 MVP 实施清单

## M1/M2 权威与协议前置修复（完成，2026-08-04）

- [x] P6.1：以失败测试锁定设置、重置与启动只经 `BehaviorPlanner` 重锚并由 Rust 应用窗口位置。
- [x] P6.2：以失败测试锁定 v2 严格 `RuntimeSnapshot` 与两个窗口一致的旧序号拒绝。
- [x] P6.3：以失败测试锁定 Rust 专用教程气泡指令，移除前端语料、`pet_clicked` 与 `pet://bubble`。
- [x] P6.4：运行 TypeScript、Rust、共享夹具和完整回归；记录 Windows 验收豁免。

### P6 执行约束

- 本轮采用破坏性 `protocolVersion: 2` 迁移；运行时不保留 v1 回退。
- 本轮只修复 M1/M2 权威与协议前置条件，不实现 M3 语料、权重、冷却、ambient 或其他互动策略。
- 用户明确豁免本轮 M2 原生 Windows 手工验收；该验收仍未执行，不能标记为已通过。

### P6 TDD、回归与审查记录

- P6.2 红：`npm test -- src/runtime-snapshot-receiver.test.ts` 因缺少 `runtime-snapshot-receiver` 模块失败；绿：共享接收器拒绝旧序号，`PlanPlayer` 和教程气泡均复用它。
- P6.3 红：`CARGO_TARGET_DIR=/tmp/yunwei-p6-red-model cargo test --manifest-path src-tauri/Cargo.toml tutorial_bubble_directive_requires_a_v2_id_sequence_and_consistent_visibility` 因 `TutorialBubbleDirective` 不存在报 E0422；绿：v2 指令模型、Rust 文案生产与 TypeScript 呈现器通过。
- P6.1：`planner_placement` 纯测试确认设置锚点在 planner footing 上；启动、更新、重置均先 `reanchor`，仅 `apply_m1_position` 写入窗口位置。
- 协议：MotionPlan、RuntimeSnapshot、输入、动画与教程指令统一为 v2；夹具更新为 v2，v1/未知版本由严格解析安全拒绝。
- 回归：`npm run check`（8 文件、28 用例）通过；默认 `cargo test --manifest-path src-tauri/Cargo.toml` 仍受既有 `/home/adduser/desktop` Tauri 权限缓存路径阻断，隔离 `CARGO_TARGET_DIR=/tmp/yunwei-p6-verify cargo test --manifest-path src-tauri/Cargo.toml`（23 用例）通过；`git diff --check` 通过。
- 审查：两路只读审查先发现 bubble 启动晚于隐藏快照会误显示、以及 `SettingsPatch.tutorialStep` 可造成指令不同步；已改为启动补拉并序号消费权威快照，且移除该外部设置字段并补回归。README 与架构协议说明同步为 v2、无旧事件回退。
- Windows：按用户明确指示，本轮未执行 M2 原生 Windows 10/11 手工验收；M2.6 继续保持未完成。

## M2 生活在 Windows 里（自动验证完成；Windows 原生验收待执行，2026-08-04）

- [x] M2.1：以 fake `EnvironmentPort` / `WindowPort` 建立环境快照、纯策略与薄 Windows 适配层。
- [x] M2.2：完成多显示器、负坐标、当前显示器约束及每屏 DPI 逻辑/物理换算。
- [x] M2.3：完成前台窗口、窗口顶部落脚、移动跟随、全屏与精确指定应用隐藏。
- [x] M2.4：完成普通窗口上方/仅桌面模式、隐藏恢复和显示器断开安全恢复。
- [x] M2.5：完成版本化环境快照接线、TypeScript 消费、共享协议夹具与安全降级。
- [ ] M2.6：原生 Windows 手工验收（100%/150% 双屏、移动/缩放、全屏、指定应用、模式、拔插、睡眠唤醒）待在 Windows 主机执行；自动回归已通过。

### M2 执行约束

- Rust 独占环境采集、显示器/落脚点/可见性决策与窗口布局；Canvas 仅播放 Rust 计划、绘制和上报输入事实。
- 默认 `AboveNormalWindows`、空隐藏规则；M2 使用仅内存 Rust 开发命令切换策略，不新增 M4 设置 UI 或持久化。
- 当前显示器：拖拽时取宠物窗口主屏；普通窗口上方时取前台窗口物理重叠最大的主屏；其余使用最后有效宠物屏，失效回退主屏。
- 指定应用精确匹配：优先 AUMID，回退规范化完整 exe 路径；禁止标题模糊匹配。

### M2 自动验证与验收记录

- Rust 红绿：`environment::tests::m2_*` 覆盖双屏负坐标/150% DPI、前台顶部、全屏优先级、指定应用、模式和断屏恢复；`model::tests::m2_*` 消费共享夹具与旧协议默认值。
- TypeScript：`pet-model` 与 `PlanPlayer` 覆盖环境快照解析、前端隐藏/恢复、计划替换、旧序号和未知可见性原因安全降级。
- 共享夹具：`tests/fixtures/protocol/m2-dual-monitor-150.json` 由 Rust/TS 共用；其余环境矩阵由纯 Rust fake 快照覆盖。
- 已验证：`npm run check`（6 文件、24 用例）与 `CARGO_TARGET_DIR=/tmp/yunwei-m2-final-target cargo test --manifest-path src-tauri/Cargo.toml`（20 用例）通过。
- Windows 原生验收：当前 Linux/WSL 无法证明 Win32 窗口行为；待在 Windows 10/11 实机按 M2.6 清单执行并补充系统版本、布局、缩放、前台应用与证据。

## M1 → M2 前置修复（2026-08-04）

- [x] P1：移除运行时旧 `BehaviorEngine` / `pet://state` / `move_pet` 权威回退，初始化失败仅安全失败。
- [x] P2：拖拽开始生成递增 sequence、planId 的 `DragVisual` 权威计划；结束由 Rust 生成落地计划。
- [x] P3：完整化 Rust/TypeScript `RuntimeSnapshot`、输入与动画观察协议，并以实时 `position_at(nowMs)` 纠偏。
- [x] P4：补齐启动、行走/跳跃纠偏、拖拽开始/结束、非法 facing/旧序号/未知版本共享夹具与消费者测试。
- [x] P5：完成局部回归、完整 `npm run check` / Cargo 回归和 M2 前置复核。

### M1 前置修复 TDD 证据

- P2 红：`cargo test --manifest-path src-tauri/Cargo.toml behavior::tests::drag_start_replaces_the_active_plan_with_a_new_drag_visual_plan` 因 `begin_drag` 无计划返回值及缺少 `MotionKind::Drag` 编译失败；绿：同命令通过。
- P3 红：`npm test -- src/pet-model.test.ts` 因缺少 `parseRuntimeSnapshot` 失败；绿：严格快照、非法 facing 拒绝通过。
- 局部绿：`npm test -- src/pet-model.test.ts src/plan-player.test.ts`（8 用例）与隔离 Cargo planner/快照测试通过。
- 完整绿：`npm run check`（6 文件、23 用例）通过；`cargo test --manifest-path src-tauri/Cargo.toml` 的既有 target 仍被 `/home/adduser/desktop` 旧绝对路径缓存阻断；隔离 `CARGO_TARGET_DIR=/tmp/yunwei-m1-prereq-final-target` 的同一命令 14 个 Rust 测试通过；`git diff --check` 通过。

### M1 前置修复 Review

- 已删除运行时旧行为/位置回退；`RuntimeData` 持有非可选 planner，拖拽与落地均由 planner 生成计划。未新增任何 M2 Windows 环境能力。

## M1 自然地活着（2026-08-04）

- [ ] M1.1 领域模型：版本化行为、坐标、落脚点与运动计划
- [ ] M1.2–M1.3 行为规划与权威位置：可注入时钟/RNG、冷却、跳跃和落脚
- [ ] M1.4 运动协议：共用夹具、Rust 生产者、TypeScript 解析与非法消息处理
- [ ] M1.5 Canvas 计划播放：PlanPlayer、阶段资源契约与连续插值
- [ ] M1.6 拖拽控制权：前端只报告事实，Rust 归还到权威落脚点
- [ ] M1.7 资源契约与 Windows 验收记录

### M1 TDD 证据

- M1.1 红：`cargo test --manifest-path src-tauri/Cargo.toml model::tests::m1_motion_plan_requires_a_contiguous_phase_schedule` 因缺少 MotionPlan/WorldPoint/阶段类型编译失败；绿：同一模型测试通过。
- M1.2 红：`cargo test --manifest-path src-tauri/Cargo.toml behavior::tests::m1_planner_prevents_consecutive_jumps_and_applies_jump_cooldown` 因缺少 planner/RNG/Footing 类型失败；绿：修正跳跃冷却后通过。
- M1.5 红：`npm test -- src/plan-player.test.ts` 因缺少 PlanPlayer 模块失败；绿：计划插值、旧序号丢弃与共用夹具消费通过。
- M1.7 红：`npm run assets:check` 发现 jumpDescend 帧不属于既有 tumble 动画；绿：修正为既有帧后通过。
- 完整回归：`npm run check`（6 文件/21 用例）与隔离 `CARGO_TARGET_DIR` 的 Rust `--all-targets`（16 用例）通过；`git diff --check` 通过。

### M1 Review

- 自动验证完成；Windows 手工验收未执行，当前 WSL/Linux 环境不能替代。

## 里程碑开发文档（2026-08-04）

- [x] 核对当前实现与目标架构的差距
- [x] 创建产品、架构、Codex 规则和测试策略文档
- [x] 创建 M1-M4 里程碑执行文档
- [x] 检查每份里程碑文档覆盖 19 项必需内容并复核目录/链接

### 文档边界

- 本轮只允许修改 `docs/` 与本任务记录；不改 Rust/TypeScript 业务代码、依赖、素材或产品规则。
- 技术栈固定为 Tauri 2、Rust 2021、TypeScript 5、Vite 6、Canvas 2D、`tauri-plugin-autostart`、`tauri-plugin-single-instance`。
- M1-M4 是完整执行单位；每个模块遵守 TDD，Windows 特性采用薄适配层、可测试核心和手工验收组合。

### Review

- 已创建 `docs/PRODUCT_SPEC.md`、`docs/ARCHITECTURE.md`、`docs/CODEX_RULES.md`、`docs/TEST_STRATEGY.md` 和四份 `docs/milestones/M*.md`。
- 每份里程碑文档均已检查为 19 个必需章节；用户列出的 M1-M4 功能关键词全部覆盖。
- 文档路径、当前模块名与现有 Tauri 双窗口/IPC/测试结构已交叉核对；`git diff --check` 通过。未修改业务代码、依赖或素材，也未开始 M1 实现。

## README 技术栈梳理（2026-08-04）

- [x] 盘点前端、Rust/Tauri、资源、测试与 Windows 打包配置
- [x] 根据实际实现重写项目 README
- [x] 校验 README 中的文件路径、脚本命令与技术描述

### 编写约束

- 只描述仓库内已实现或已配置的能力；Windows 真实运行与安装包验收的边界须明确说明。
- README 面向首次接手项目的开发者：先说明项目与架构，再给出环境、开发、测试和发布操作。

### Review

- `npm run check` 通过：Tauri/NSIS 配置、8 个图集、2 个 WAV、TypeScript、5 个 Vitest 文件（18 个用例）和 Vite 生产构建均成功。
- `cargo test --manifest-path src-tauri/Cargo.toml` 的既有 `src-tauri/target` 缓存包含指向 `/home/adduser/desktop` 的过期 Tauri 生成路径，首次验证因此失败；使用全新隔离 target 重跑同一命令后，12 个 Rust 用例全部通过。该问题未改动项目源码或配置。
- 已逐项核对 README 记载的仓库路径、npm 脚本与配置，并通过 `git diff --check`。

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
