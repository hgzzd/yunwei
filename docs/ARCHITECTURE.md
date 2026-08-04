# 云尾兽目标架构

## 架构总则

系统坚持一个方向的数据流：**Windows 环境和用户输入 → Rust 决策/权威状态 → 明确协议 → TypeScript/Canvas 呈现**。前端可以报告事实（指针、资源就绪、动画完成），不能把事实升级成高层行为决定。

```text
Windows API / Tauri 窗口信息 / 用户输入
                    │
                    ▼
┌──────────────── Rust 2021 ────────────────┐
│ EnvironmentPort  PositionAuthority          │
│ BehaviorPlanner  LandingResolver             │
│ InteractionPolicy SettingsStore               │
│                                                │
│ command: 前端报告输入或请求设置                │
│ event: 发送权威快照、运动计划、气泡命令        │
└───────────────┬──────────────────────────────┘
                │ Tauri IPC（版本化 JSON 载荷）
     ┌──────────┴──────────┐
     ▼                     ▼
pet 窗口                 bubble 窗口
TypeScript + Canvas      TypeScript + DOM/CSS
动画与微动作             文本、位置、显示时长
```

## 运行时边界

### Rust（权威层）

- `BehaviorPlanner`：选择高层动作、运动计划、冷却和中断，不暴露给前端自由切换。
- `PositionAuthority`：保存逻辑坐标、所在显示器、落脚点、窗口矩形和恢复策略。
- `EnvironmentPort`：薄 Windows 适配层，读取显示器、DPI、前台窗口、全屏、窗口矩形与应用标识；纯规则不放入 Win32 FFI。
- `InteractionPolicy`：把前端上报的单击、悬停、抚摸、拖拽等事实转换为可接受的交互结果。
- `SettingsStore`：版本化设置、归一化、原子写入、迁移和安全回退。
- Tauri：管理两个窗口、托盘、自动启动、单实例、IPC 和打包。

### TypeScript + Canvas（表现层）

- `InputSampler`：归一化 Pointer Event，识别前端可观测的手势事实并发送给 Rust。
- `PlanPlayer`：接收 Rust 的 `MotionPlan`/`RenderDirective`，按时间戳插值，驱动 Canvas 帧、翻转与局部微动作；不能生成目标坐标或替代动作。
- `SpriteRenderer`：图集加载、帧选择、DPI Canvas 和资源失败降级。
- `BubblePresenter`：只显示 Rust 指令指定的类别/文本/时长/锚点；不自选语料。
- `tauri-bridge.ts`：唯一 IPC 边界，负责协议版本和载荷校验。

## 两层状态机

| 层 | 所在端 | 示例 | 约束 |
| --- | --- | --- | --- |
| 高层行为状态机 | Rust | Idle、Walk、Jump、Land、Dragged、Hidden | 决定意图、允许迁移、计划、冷却和中断 |
| 表现状态机 | TypeScript/Canvas | 起步、腾空上行、顶点、下落、落地缓冲、眨眼、尾巴摆动 | 只能在 Rust 下达行为范围内播放；动画结束可回报事实 |

`PetState`、`BehaviorEngine`、`PetSettings`、`SettingsStore`、`platform.rs`、`runtime.ts`、`SpriteRenderer`、`GestureTracker` 是当前仓库的基础。后续实施应在这些清晰边界附近拆分模块，不把新的规划继续堆进 `src-tauri/src/lib.rs`。

## 目标协议

所有跨端载荷均使用 `camelCase`、显式 `protocolVersion`、稳定 ID、单调 `sequence` 和 Rust 时间戳。未知版本、非法枚举或过期序号必须安全忽略并记录诊断。

```ts
type NativeToWeb =
  | { protocolVersion: 2; sequence: number; kind: "renderDirective"; directive: RenderDirective }
  | { protocolVersion: 2; sequence: number; kind: "motionPlan"; plan: MotionPlan }
  | { protocolVersion: 2; sequence: number; kind: "bubbleDirective"; bubble: BubbleDirective }
  | { protocolVersion: 2; sequence: number; kind: "runtimeSnapshot"; snapshot: RuntimeSnapshot };

type WebToNative =
  | { protocolVersion: 2; kind: "inputObserved"; observation: InputObservation }
  | { protocolVersion: 2; kind: "animationObserved"; observation: AnimationObservation }
  | { protocolVersion: 2; kind: "settingsRequested"; patch: SettingsPatch };
```

本次协议采用破坏性 v2 切换：生产者、消费者和共享夹具同步升级，运行时不保留 `pet://state`、`pet://bubble` 或 v1 兼容回退路径。

## 坐标与窗口模型

- 逻辑世界坐标：Rust 定义，以显示器工作区和逻辑像素表达；保存时使用显示器稳定标识及归一化水平位置。
- 物理窗口坐标：仅在 `EnvironmentPort`/窗口适配器中进行 DPI 换算并调用 Tauri/Win32。
- 落脚点：Rust 给出的可站立矩形顶边和允许横向区间；Canvas 不从截图、DOM 或鼠标位置推断落点。
- 宠物与气泡分别拥有窗口矩形；气泡位置由 Rust 锚定宠物或环境上下文，前端只布局窗口内部内容。

## Windows 适配原则

Windows API 必须藏在窄 trait 后，例如 `EnvironmentPort`、`WindowPort`、`ForegroundWindowPort`。领域模型和策略接收值对象/快照，因此可以在 Rust 单测中以 fake/mock 覆盖。原生调用只做读取、转换和错误映射，不承载策略分支。

## 现有代码与目标差距

当前 `BehaviorEngine` 已含随机状态、拖拽和点击，`lib.rs` 已持有位置与全屏逻辑；但尚未形成分离的高层计划/表现状态机、版本化运动协议、完整跳跃阶段、可替换 Windows 环境端口、前台窗口落脚策略、互动语料策略、设置页和结构化日志。M1–M4 依次收敛这些差距。
