# M3：成为有性格的朋友

## 1. 里程碑目标

在不改变低打扰定位的前提下，让宠物能把用户互动和活动状态转化为有限、可解释、低频的反馈，并始终通过独立气泡窗口呈现语言。

## 2. 用户完成后能感知到的结果

用户单击、连续点击、悬停、抚摸或拖拽时，会得到符合当下状态的轻量反馈；宠物偶尔主动说一句，但不会重复刷屏、抢焦点或变成聊天窗口。

## 3. 实现范围

单击反馈、连续点击识别、鼠标悬停、抚摸识别、拖拽人格反馈、独立气泡窗口、分类语料库、情境触发、权重选择、去重/冷却、低频随机说话和活动状态感知。

## 4. 明确不实现的范围

- 不实现自由文本聊天、LLM、联网语料、用户账号、消息输入框或主动通知系统。
- 不让 TypeScript 根据点击次数直接选台词/行为；前端只上报互动事实。
- 不在本里程碑实现设置页中的说话频率控制（M4），但设计可配置的策略入口。
- 不改变 M2 的环境识别和显示模式决策。

## 5. 内部模块拆分

| 模块 | 责任 |
| --- | --- |
| M3.1 输入观察 | 将单击、连续点击、停留、抚摸轨迹、拖拽阶段归一化为事实 |
| M3.2 互动策略 | Rust 依据宠物状态、活动状态和冷却接受/拒绝反馈 |
| M3.3 活动状态 | 从 M1/M2 权威快照导出忙碌、空闲、隐藏等有限上下文 |
| M3.4 语料库 | 分类条目、权重、触发条件、去重键和文本版本 |
| M3.5 选择器 | 过滤、加权抽样、最近历史、全局/类别冷却 |
| M3.6 气泡控制器 | Rust 决定内容/时长/锚点；bubble 窗口显示、替换和关闭 |

## 6. 模块之间的依赖顺序

`M3.1 输入观察` → `M3.3 活动状态` → `M3.2 互动策略` → `M3.4 语料库 + M3.5 选择器` → `M3.6 独立气泡接线`。不可先在前端写随机台词，再补 Rust 语料策略。

## 7. 需要新增或修改的数据结构

```text
InteractionKind = SingleClick | RepeatedClick | Hover | Petting | DragStarted | DragEnded
InputObservation { occurredAtMs, kind, pointer?, durationMs?, count?, pathLength? }
ActivityState = Idle | Moving | Jumping | Dragged | Hidden | EnvironmentSuppressed
CorpusCategory = Greeting | Click | Petting | Drag | Ambient
Line { id, category, text, weight, requiredActivities, cooldownMs, dedupeKey }
SpeechContext { activity, interaction?, visibilityReason?, nowMs }
BubbleDirective { id, category, text, anchor, durationMs, replacePolicy }
SpeechHistory { recentLineIds, categoryLastShownAt, globalLastShownAt }
```

现有 `BubblePayload`/`BubbleMessage` 和教程气泡需迁移为兼容包装：教程仍是专门指令，不与随机语料竞争同一冷却池。

## 8. Rust 与 TypeScript 的职责边界

TypeScript 用确定阈值采样 Pointer Event，报告单击数、悬停时间、抚摸路径长度和拖拽阶段；它不判断“宠物该说什么”。Rust 根据 `SpeechContext`、语料权重、去重/冷却和当前显示条件选择反馈，发送 `BubbleDirective`。bubble 前端只显示 Rust 给定文本、可见性和时长。

## 9. 事件和通信协议

| 方向 | 名称/载荷 | 语义 |
| --- | --- | --- |
| pet → Rust | `input_observed` / `InputObservation` | 互动事实，含单调时间与可选指标 |
| Rust → bubble | `pet://bubble-directive` / `BubbleDirective` | 已选择的台词、锚点、时长和替换策略 |
| Rust → pet | `pet://interaction-feedback` | 可选表现指令，如对点击的既定动作反馈 |
| Rust → 两窗口 | `runtime-snapshot` | 当前 `ActivityState` 与显示限制 |

`BubbleDirective.id` 保障迟到的关闭计时器不隐藏新内容；前端不得从文本为空推测新的高层状态。

## 10. TDD 测试计划

输入观察、互动策略、语料选择器和气泡呈现分别先红测。每个时间规则注入时钟；每个权重规则注入可重复 RNG。先完成纯策略和夹具，再连入真实 pointer 与 bubble 窗口。

## 11. Rust 单元测试

- 相同 `InputObservation` 在不同 `ActivityState` 下是否应答符合策略；隐藏/环境抑制时不发气泡。
- 给定固定 RNG，权重选择在合法候选中确定；冷却、去重、空候选和分类回退正确。
- 连续点击、拖拽和低频 ambient 使用独立冷却，且不会越过全局低打扰限制。
- bubble 替换/关闭按 `id` 生效，过期关闭不影响新指令。

## 12. TypeScript 单元测试

- 输入采样把单击、连续点击、悬停、抚摸和拖拽报告为正确事实，取消/跨指针不误触发。
- `BubblePresenter` 对显示、替换、计时关闭、隐藏快照与过期指令确定响应。
- 未知 `CorpusCategory`/协议版本或非法时长安全忽略；前端没有候选台词选择逻辑。

## 13. 协议或集成测试

用统一夹具覆盖：单击→反馈、连续点击触发不同分类、抚摸、拖拽、隐藏期互动、冷却拒绝、重复文案、ambient 随机和新气泡覆盖旧气泡。测试 Rust 选择结果与 TS 气泡呈现，验证 `id` 和序号顺序。

## 14. Windows 手工验收

在正常桌面连续操作：单击、快速连续点击、悬停、缓慢抚摸、拖拽/松开；观察气泡是否始终在独立窗口、无焦点抢占且不拦截鼠标。再在全屏/指定应用隐藏状态下互动，确认无不当出声或气泡；至少观察一个随机说话周期，检查频率与重复。

## 15. 验收标准

- 所有台词选择发生于 Rust，TS 只报告/呈现；代码和协议测试均能证明。
- 分类、权重、去重、类别冷却、全局冷却和低频随机均由可重复单测覆盖。
- 气泡窗口始终独立、透明、小型、不可抢焦点；不存在全屏透明交互层。
- 互动与隐藏情况下的 Windows 手工验收通过，未引入高频打扰。

## 16. 风险

- “抚摸”阈值若过低会把普通移动误判；若过高又不可发现，必须通过夹具与手工样本校准。
- 语料权重无法替代冷却；缺少全局限流会破坏低打扰约束。
- 教程和常规气泡共享窗口时，替换策略可能吞掉引导，需使用明确优先级而非随机覆盖。

## 17. 回滚方案

保留现有教程气泡的独立兼容指令。互动策略/语料异常时 Rust 返回“无气泡”而非由 TS 选择默认文案；任何 bubble 呈现错误都只隐藏 bubble，不影响宠物窗口或 M1/M2 运动。

## 18. 建议的提交顺序

1. `test(m3): add input-observation and interaction-policy red tests`
2. `feat(m3): report normalized interactions to Rust`
3. `test(m3): add corpus, weighted selection and cooldown fixtures`
4. `feat(m3): add native speech policy and bubble directives`
5. `test(m3): add independent bubble presenter integration tests`
6. `feat(m3): connect low-frequency ambient speech and manual acceptance records`

## 19. Codex 完成报告格式

```md
## M3 完成报告
- 范围：已实现互动、语料和气泡模块；明确未实现项。
- 边界证明：前端上报的事实列表；Rust 选择的行为/台词列表。
- TDD：红测→最小实现→重构→回归的命令和结果。
- 自动验证：冷却/去重/权重、输入采样、协议夹具、完整回归。
- Windows 验收：交互步骤、气泡焦点/穿透表现、隐藏状态表现和证据。
- 风险/回滚：语料或窗口异常时的无气泡安全退化。
```

