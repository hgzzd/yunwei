# M2：生活在 Windows 里

## 1. 里程碑目标

在 M1 的权威运动模型上接入 Windows 环境，让宠物正确理解显示器、DPI、前台窗口、全屏与显示模式，并始终有安全恢复位置。

## 2. 用户完成后能感知到的结果

在多显示器、缩放比例不同、窗口移动、全屏应用或显示器断开时，宠物不会漂移、遮挡或消失；它按当前模式只在允许的桌面/窗口上下文中出现和落脚。

## 3. 实现范围

多显示器、当前显示器约束、DPI/坐标换算、前台窗口识别、前台窗口顶部落脚、窗口移动跟随、全屏检测、指定应用隐藏、普通窗口上方模式、仅桌面显示模式及显示器断开恢复。

## 4. 明确不实现的范围

- 不修改 M1 的高层行为决策权和运动协议。
- 不实现新的性格互动、气泡语料（M3）。
- 不实现设置页面/托盘产品化交互（M4）；本里程碑只定义可由后续设置持久化的模式数据。
- 不实现全屏透明覆盖层、屏幕截图识别或未确认的应用自动化。

## 5. 内部模块拆分

| 模块 | 责任 |
| --- | --- |
| M2.1 环境端口 | `EnvironmentPort`/`WindowPort` trait 与 Win32/Tauri 薄适配 |
| M2.2 显示器模型 | `MonitorSnapshot`、工作区、稳定 ID、当前显示器选择和断开恢复 |
| M2.3 坐标换算 | 逻辑/物理坐标、每屏 DPI、窗口矩形转换 |
| M2.4 前台模型 | 前台窗口身份、矩形、可见性、全屏与指定应用匹配 |
| M2.5 落脚策略 | 桌面基础、前台顶部、普通窗口上方、仅桌面显示的策略 |
| M2.6 跟随/恢复 | 窗口移动重规划、全屏/指定应用隐藏、断屏安全恢复 |

## 6. 模块之间的依赖顺序

`M2.1` → `M2.2 + M2.3` → `M2.4` → `M2.5` → `M2.6`。任何 UI 模式切换不得早于可测试的环境快照和落脚策略。

## 7. 需要新增或修改的数据结构

```text
MonitorSnapshot { id, workAreaLogical, workAreaPhysical, scaleFactor, isPrimary }
ForegroundWindowSnapshot { appId?, title?, rectPhysical, monitorId?, visible, isFullscreen }
DisplayMode = AboveNormalWindows | DesktopOnly
HideRule { appId }                 // 指定应用隐藏；匹配规则由用户设置提供
EnvironmentSnapshot { monitors, foreground?, capturedAtMs }
FootingSource = DesktopWorkArea | ForegroundWindowTop
RecoveryPosition { monitorId?, normalizedX, fallback: PrimaryWorkArea }
```

现有 `MonitorArea`、`platform::foreground_is_fullscreen` 和 `PetSettings.monitor_id/normalized_x` 要迁移为这些领域值对象的适配来源，不复制第二套坐标真相。

## 8. Rust 与 TypeScript 的职责边界

Rust 轮询/订阅环境、识别前台窗口、做 DPI 换算、选显示器/模式/落脚点、发起或中断 M1 计划并移动窗口。TypeScript 只把收到的最终窗口/运动状态绘制出来；不读 Win32、不用浏览器屏幕 API 推测前台窗口，也不自行隐藏整个页面。

## 9. 事件和通信协议

| 方向 | 名称/载荷 | 语义 |
| --- | --- | --- |
| 环境适配 → Rust | `EnvironmentSnapshot` | 薄适配层输出的纯快照 |
| Rust → pet/bubble | `pet://runtime-snapshot` | 带 `displayMode`、`footing`、`visibilityReason` 的权威快照 |
| Rust → pet | `pet://motion-plan` | 窗口移动或落脚变更后的新计划 |
| M4 设置请求预留 | `settingsRequested { displayMode, hideRules? }` | 本阶段定义验证，M4 才提供设置 UI |

隐藏原因必须枚举化（`fullscreen`、`specifiedApp`、`desktopOnlyForeground`、`monitorUnavailable`），不能用一个无来源的布尔值覆盖。

## 10. TDD 测试计划

先对 fake `EnvironmentPort` 写显示器、DPI、前台、落脚、恢复和优先级的红测；再实现纯策略；最后把 `platform.rs` 扩为薄适配并执行 Windows 场景。每个模式策略单独覆盖，避免通过真实 API 难测而合并成巨型函数。

## 11. Rust 单元测试

- 同布局多屏/负坐标/不同 DPI 下，逻辑↔物理换算和当前显示器选择可逆或按规范夹紧。
- 前台窗口顶部落脚只在普通窗口上方模式且窗口/显示器有效时产生；桌面模式忽略其顶部。
- 全屏、指定应用、桌面模式前台优先级可预测，手动隐藏优先级不回归。
- 显示器断开时恢复到主显示器工作区的安全归一化位置，绝不保留无效屏 ID。
- 窗口矩形改变时重规划而非直接让前端偏移。

## 12. TypeScript 单元测试

- 解析含 `displayMode`、`visibilityReason`、不同 DPI 结果的快照；旧序号/未知原因安全降级。
- 收到隐藏快照时停止 Canvas 绘制与气泡展示；恢复后只按 Rust 最新计划继续。
- `PlanPlayer` 在窗口跟随计划替换时无本地目标累积或坐标漂移。

## 13. 协议或集成测试

夹具覆盖：双屏 100%/150%、负坐标副屏、前台窗口跨屏、全屏、指定应用、桌面模式、显示器拔除。由 Rust 策略输出快照，TS 消费者验证呈现命令；模拟 Tauri 事件顺序验证旧快照不会覆盖新恢复。

## 14. Windows 手工验收

在原生 Windows 10/11 测试：100%+150% 双屏（含左侧负坐标）；在每屏拖放后切换前台普通窗口、移动/调整其位置、最大化/全屏；为一个已配置应用触发隐藏；切换普通窗口上方/仅桌面模式；运行中拔插副屏并从睡眠唤醒。逐项记录布局、预期和实际。

## 15. 验收标准

- 每个环境策略都由 mock 单测覆盖，Win32 调用位于薄端口层。
- 两种显示模式、全屏、指定应用隐藏和断屏恢复都发出可解释的 `visibilityReason`。
- 多 DPI 多屏下位置、宠物大小、气泡锚点和落脚点无明显漂移；Windows 手工场景全通过。
- M1 行为计划仍由 Rust 决定，前端没有新增环境决策。

## 16. 风险

- Windows 前台窗口、UWP/权限提升窗口和虚拟桌面可用信息不一致。
- “指定应用”需要稳定应用标识；不能以模糊标题匹配创造未确认规则。
- DPI awareness 或坐标空间混用会造成双屏偏移。

## 17. 回滚方案

环境端口失败时返回“不可用”快照，策略退回当前可用显示器工作区基础落脚点；不读取陈旧物理坐标。新模式设置在 M4 持久化前保持默认 `AboveNormalWindows`，并可由 Rust 回退到既有全屏隐藏行为。

## 18. 建议的提交顺序

1. `test(m2): add environment snapshots and coordinate policy red tests`
2. `feat(m2): extract Windows environment ports and monitor model`
3. `test(m2): add foreground footing and visibility policy tests`
4. `feat(m2): plan against foreground windows and display modes`
5. `test(m2): add monitor disconnect and protocol fixtures`
6. `feat(m2): connect recovery and Windows manual acceptance records`

## 19. Codex 完成报告格式

```md
## M2 完成报告
- 范围：完成的环境端口、策略和模式；未实现项。
- 规则证据：fake 环境快照覆盖的显示器/DPI/前台/隐藏/恢复矩阵。
- IPC：新增或迁移的载荷、版本与兼容策略。
- 自动验证：Rust、TS、协议夹具及完整回归命令/结果。
- Windows 验收：系统版本、显示器布局、缩放、前台应用、逐项结果和证据。
- 风险/回滚：适配失败与端口不可用时的安全退化。
```

