# M1：自然地活着

## 1. 里程碑目标

建立“Rust 决策、Canvas 连续呈现”的生命基础：宠物可在明确落脚点上自然待机、行走、完成一次有起跳/腾空/落地缓冲的跳跃，并在拖拽时正确切换控制权。

## 2. 用户完成后能感知到的结果

用户看见的是连续、有重量感的桌宠：它不会随机瞬移或凭前端猜测目的地；走动有路线，跳跃有完整阶段，落地有缓冲，拖动后能平稳回到 Rust 确认的落脚点。

## 3. 实现范围

- 双层状态机：Rust 高层 `BehaviorState` 与前端 `PresentationPhase`。
- Rust↔前端版本化行为协议、权威坐标、运动计划和插值契约。
- 自然待机、行走、完整跳跃（准备/起跳/上行/顶点/下落/落地）和落地缓冲。
- 基础落脚点、行为调度/冷却、拖拽控制权切换和基础动作资源规范。

## 4. 明确不实现的范围

- 不实现前台窗口顶部落脚、多屏策略、DPI 策略、隐藏模式（M2）。
- 不实现抚摸、语料、情境说话（M3）。
- 不实现设置页、智能命中、日志、打包发布功能（M4）。
- 不更换既有透明双窗口或让前端选择下一高层行为。

## 5. 内部模块拆分

| 模块 | 责任 | 建议落点 |
| --- | --- | --- |
| M1.1 领域模型 | `BehaviorState`、`PresentationPhase`、`WorldPoint`、`Footing`、`MotionPlan` | 新 Rust 领域模块，扩展 `model.rs` |
| M1.2 行为规划 | 待机/行走/跳跃选择、冷却、可中断迁移 | 从 `behavior.rs` 演进为 planner |
| M1.3 权威位置 | 当前坐标、目标落脚点、拖拽接管/归还 | 从 `lib.rs` 的位置函数抽出 |
| M1.4 运动协议 | 版本、序号、时间戳、计划和确认事件 | Rust 模型 + `pet-model.ts`/`tauri-bridge.ts` |
| M1.5 表现播放 | 按计划插值、阶段驱动图集、微动作 | `runtime.ts` 分出 `plan-player.ts` |
| M1.6 输入控制 | 报告拖拽事实，不提交最终坐标决定 | 演进 `gesture.ts` |
| M1.7 资源规范 | 行为/阶段到图集帧、锚点、循环规则 | `manifest.json` 模式与校验脚本 |

## 6. 模块之间的依赖顺序

`M1.1 领域模型` → `M1.2 行为规划 + M1.3 权威位置` → `M1.4 运动协议` → `M1.5 表现播放` → `M1.6 拖拽接线` → `M1.7 资源验收`。不得先让前端做本地跳跃再回填 Rust 状态。

## 7. 需要新增或修改的数据结构

```text
BehaviorState = Idle | Walking | Jumping | Landing | Dragged
PresentationPhase = IdleLoop | WalkCycle | JumpPrepare | JumpAscend |
                    JumpApex | JumpDescend | LandCompress | LandRecover | DragVisual
WorldPoint { monitorId, xLogical, yLogical }
Footing { id, topYLogical, minXLogical, maxXLogical, source: DesktopWorkArea }
MotionPlan { id, kind, startedAtMs, durationMs, from, to, arc?, facing, phaseSchedule }
RuntimeSnapshot { protocolVersion, sequence, behavior, position, footing, activePlan? }
InputObservation = DragStarted | DragMoved { pointer } | DragEnded { pointer } |
                   AnimationCompleted { planId, phase }
```

现有 `PetState`、`StatePayload`、`MonitorArea`、`DragState` 是迁移基础；M1 完成后旧载荷只可作为兼容适配，不可并行成为第二权威状态。

## 8. Rust 与 TypeScript 的职责边界

Rust 选择行为、时长、路径、朝向、落脚点、坐标和拖拽结束位置；TypeScript 以收到的 `MotionPlan` 为时间轴，在 Canvas 上逐帧表现和可选视觉微动作。前端仅报告指针与动画完成事实，不得自行改写 `to`、跳跃弧线、下一个行为或落脚点。

## 9. 事件和通信协议

| 方向 | 名称/载荷 | 语义 |
| --- | --- | --- |
| Rust → pet | `pet://motion-plan` / `MotionPlan` | 开始/替换一个权威运动计划 |
| Rust → pet | `pet://runtime-snapshot` / `RuntimeSnapshot` | 重连、拖拽归还或纠偏时的权威快照 |
| pet → Rust | `input_observed` / `InputObservation` | 报告拖拽开始、移动、结束；不申请行为 |
| pet → Rust | `animation_observed` | 报告计划阶段结束，Rust 决定是否迁移 |

所有消息携带 `protocolVersion: 1`；前端按 `sequence` 丢弃旧快照，Rust 以 `planId` 忽略迟到的动画完成事件。

## 10. TDD 测试计划

按 5 个模块分别红测：领域/迁移、规划与冷却、坐标与落脚、协议、播放/拖拽。每个模块遵循失败测试→确认失败→最小实现→局部绿→重构→完整回归；资源仅在协议和状态通过后接入。

## 11. Rust 单元测试

- 固定 RNG/时钟下，允许的高层迁移、冷却和拖拽中断确定且无非法跳转。
- 给定 `Footing` 与初始点，行走和跳跃计划起终点均在允许范围；跳跃阶段顺序完整且落地回到 `Landing`。
- 拖拽开始后 planner 停止自动迁移；结束时只接受 Rust 计算的最近有效基础落脚点。
- 过期 `planId`、非法坐标和损坏 `MotionPlan` 不改变权威状态。

## 12. TypeScript 单元测试

- `PlanPlayer` 对给定时间戳插值，阶段边界和最终帧可预测。
- 收到同一/较旧 `sequence` 不倒退；未知协议或非法计划安全忽略。
- `GestureTracker` 只发 `InputObservation`，不生成目标坐标或状态迁移。
- 图集资源规范：每个 M1 高层行为/表现阶段拥有可用帧、锚点与 loop 标记。

## 13. 协议或集成测试

使用同一 JSON 夹具覆盖 Rust 序列化和 TypeScript 解析：walk、jump、drag、计划替换、旧序号、未知版本、非法阶段。桥接测试证明 `MotionPlan` 到 Canvas 行为的端到端映射，不需要真实窗口。

## 14. Windows 手工验收

在一块 100% DPI 主显示器上：启动后观察待机；等待行走；观察至少一次完整跳跃并确认无瞬移；拖到工作区不同横向位置后松开；重复拖拽/中断跳跃。记录每一步的视频或日志，确认气泡窗口没有参与输入覆盖。

## 15. 验收标准

- 高层行为和最终位置均由 Rust 日志/快照证明，前端没有决策路径。
- 跳跃具备六个阶段并落到有效 `Footing`；动画与权威快照可纠偏。
- 拖拽期间自动行为停止，结束后安全落地；现有随机待机/行走不回归。
- 新增 Rust、TS、协议测试全绿，`npm run check` 与 Cargo 测试通过，Windows 手工场景通过。

## 16. 风险

- 现有 `behavior.rs` 的状态和帧概念混合，直接扩展会继续耦合。
- 图集可能缺少跳跃阶段帧；这是资源契约缺口，不能以 JS 自行发明高层状态掩盖。
- 高频 Rust 快照可能造成 Canvas 抖动；计划事件与纠偏快照需区分。

## 17. 回滚方案

以 feature-local 兼容适配保留当前 `pet://state` 渲染路径，直到新 `MotionPlan` 的生产者、消费者和测试同批完成。若新计划播放异常，Rust 发送最后有效 `RuntimeSnapshot` 并回退至 Idle；不回退为前端自主移动。

## 18. 建议的提交顺序

1. `test(m1): add behavior and motion-plan red tests`
2. `feat(m1): add authoritative behavior and footing planner`
3. `test(m1): add protocol fixtures and plan-player red tests`
4. `feat(m1): bridge motion plans to canvas presentation`
5. `feat(m1): hand drag control back to native authority`
6. `test(m1): enforce action resource contract and document Windows acceptance`

## 19. Codex 完成报告格式

```md
## M1 完成报告
- 范围：完成的模块及未做项（必须仍为 M1 范围内）。
- 决策边界：Rust 决定了什么；Canvas 只呈现了什么。
- TDD 证据：每模块的红测命令/失败原因、最小实现后绿测命令。
- 自动验证：Rust、TypeScript、协议夹具、npm/cargo 完整回归结果。
- Windows 验收：设备/DPI、步骤、预期、实际、证据位置。
- 风险与回滚：遗留风险、触发条件、回退到的最后有效快照/兼容路径。
- 未完成：仅列确实未满足的 M1 验收项；没有则写“无”。
```

