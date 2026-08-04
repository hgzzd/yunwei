# M4：产品化收尾

## 1. 里程碑目标

把已经自然、懂 Windows、具备性格的桌宠收敛为可配置、可恢复、可排错、可安装并可发布验收的 Windows 产品。

## 2. 用户完成后能感知到的结果

用户可从托盘打开设置，按意图调整命中/穿透、位置、大小、节奏和说话频率；重启、异常、显示器变化后能安全恢复，开机启动和单实例符合预期，安装包可在原生 Windows 验收。

## 3. 实现范围

托盘、设置页、智能命中、完全穿透、固定位置、尺寸档位、节奏预设、说话频率、设置持久化、安全位置恢复、开机启动、单实例、异常恢复、日志，以及 Windows 打包与发布验收。

## 4. 明确不实现的范围

- 不增加账号、云同步、遥测、自动更新服务、在线内容、支付或新互动玩法。
- 不改动 M1 Rust 决策权、M2 环境端口边界和 M3 语料选择归属。
- 不以设置页绕过低打扰规则或开放未确认的行为状态。

## 5. 内部模块拆分

| 模块 | 责任 |
| --- | --- |
| M4.1 设置领域 | 版本化 `PetSettings`、默认、补丁、校验、迁移和原子写入 |
| M4.2 设置页/托盘 | 读取权威快照、请求合法 patch、显示失败与恢复状态 |
| M4.3 输入命中模式 | 智能命中（仅可交互不透明区域）与完全穿透的窗口策略 |
| M4.4 位置与偏好 | 固定位置、尺寸档位、节奏预设、说话频率、显示模式持久化 |
| M4.5 生命周期 | 自动启动、单实例、崩溃/资源/设置异常恢复 |
| M4.6 可观测性 | 结构化本地日志、隐私边界、诊断快照 |
| M4.7 发布 | NSIS、WebView2、签名状态说明、安装/卸载验收 |

## 6. 模块之间的依赖顺序

`M4.1` → `M4.4` → `M4.2 + M4.3` → `M4.5` → `M4.6` → `M4.7`。设置 UI 不能先于可验证的 settings schema；发布验收只能在恢复、日志和生命周期稳定后开始。

## 7. 需要新增或修改的数据结构

```text
HitTestMode = Smart | ClickThrough
PositionMode = FollowFooting | Fixed
RhythmPreset = Calm | Default | Lively       // 仅为行为调度参数，不新增行为
SpeechFrequency = Quiet | Default | Reduced  // 仅为 M3 低频策略参数
SafePosition { monitorId?, normalizedX, footingPreference, lastKnownGoodAtMs }
PetSettings vNext {
  schemaVersion, scale, soundEnabled, autostartEnabled,
  hitTestMode, positionMode, safePosition, rhythmPreset, speechFrequency,
  displayMode, hideRules, tutorialStep
}
SettingsError { code, recoverable, fallbackApplied }
DiagnosticEvent { timestamp, level, component, code, fields }
```

现有 `PetSettings`、`SettingsPatch`、`SettingsStore`、`MenuHandles` 和 autostart/single-instance 插件是迁移基础；字段增加必须保留旧文件的默认/迁移路径。

## 8. Rust 与 TypeScript 的职责边界

Rust 校验设置、应用窗口 hit-test/位置/尺寸/生命周期策略、写入设置、执行插件调用、记录本地日志并发送权威设置快照。TypeScript 设置页和托盘视图只显示当前值、请求 patch、展示可恢复错误；不得直接改窗口、localStorage 或绕开 Rust 持久化。智能命中的像素/区域事实可由 Canvas 提供，但“是否穿透”最终由 Rust 窗口策略执行。

## 9. 事件和通信协议

| 方向 | 名称/载荷 | 语义 |
| --- | --- | --- |
| TS → Rust | `settingsRequested` / `SettingsPatch` | 用户请求，Rust 校验并返回结果 |
| Rust → TS | `pet://settings` / `PetSettings` | 唯一权威设置快照 |
| Rust → TS | `pet://settings-error` / `SettingsError` | 失败与是否已回退 |
| Rust → TS | `pet://diagnostic-status` | 不含隐私内容的运行状态 |
| tray → Rust | 稳定菜单 ID | 与设置页走同一 settings policy，不复制规则 |

任何 patch 返回新的完整快照或明确错误；前端不得乐观永久保存。日志不通过 IPC 上传，诊断导出若未来需要必须另行确认。

## 10. TDD 测试计划

先为 settings 迁移/验证、窗口策略、生命周期和日志写红测；使用 fake autostart、single-instance、window port 和文件系统测试恢复。设置页和托盘通过协议夹具测试同一 patch。最后进行原生 Windows 安装、重启、异常与 DPI 场景验收。

## 11. Rust 单元测试

- 每个新设置字段默认、缺失、非法值、版本迁移和损坏 JSON 回退正确；原子写入失败保留最后有效文件。
- `PositionMode`、`SafePosition`、尺寸和显示器断开恢复不产生屏外窗口。
- `HitTestMode` 和智能命中策略对输入区域/透明区产生确定窗口命令；完全穿透时无交互。
- fake autostart/single-instance/window port 覆盖启用、失败、重启和第二实例激活。
- 诊断事件脱敏、限量并在错误路径记录；日志失败不阻止宠物启动。

## 12. TypeScript 单元测试

- 设置页根据完整快照渲染，非法/失败 patch 不在 UI 留下假状态。
- 托盘事件与设置页形成相同 `SettingsPatch`，不会各自维护配置副本。
- 智能命中区域采样只报告可交互事实；完全穿透时不触发手势。
- 设置错误、恢复快照与诊断状态可见且不泄露内部路径/敏感内容。

## 13. 协议或集成测试

夹具覆盖旧版 settings、未知字段、部分 patch、写入失败、显示器丢失、第二实例启动、autostart 失败、资源加载失败和日志不可写。测试 Rust 迁移/决策、TS 设置呈现、tray 命令映射和恢复快照的一致性。

## 14. Windows 手工验收

原生 Windows 10/11 上完成：托盘与设置页逐项切换；智能命中/完全穿透；固定位置与三种尺寸；节奏/说话频率；关闭重启、异常重启、断开显示器；开机启动；重复启动；安装/卸载、缺 WebView2、100%/150% 双屏、全屏隐藏恢复及至少两小时稳定运行。记录 NSIS 输出、系统版本和所有异常日志。

## 15. 验收标准

- 设置、托盘和窗口策略只有一条 Rust 权威路径；持久化、迁移、损坏恢复和安全位置测试全绿。
- 智能命中与完全穿透不需要全屏透明覆盖层，且不会破坏独立 bubble 窗口。
- 自动启动、单实例、异常/日志的失败路径有 mock 测试和 Windows 实测。
- NSIS 安装包在原生 Windows 构建、安装、卸载和功能验收均通过；文档明确未签名状态（若仍未签名）。

## 16. 风险

- Windows 窗口命中测试在透明/高 DPI 场景表现可能与 Canvas 像素区域不一致。
- 自启动被系统策略拒绝、日志目录不可写、杀进程等真实失败需要降级而不是崩溃。
- 安装包、WebView2 和 SmartScreen 是环境验收风险，不能由 WSL 成功构建替代。

## 17. 回滚方案

设置迁移失败时保留原文件并用默认安全设置启动；位置恢复失败时放到主显示器工作区默认点；hit-test 策略失败时退回可交互安全模式并写诊断。发布异常时撤回该安装包，保留上一已验收包；不得删除用户设置。

## 18. 建议的提交顺序

1. `test(m4): add versioned settings and safe recovery red tests`
2. `feat(m4): extend native settings policy and persistence`
3. `test(m4): add tray/settings parity and hit-test policy tests`
4. `feat(m4): add settings page, tray actions and input modes`
5. `test(m4): add lifecycle, logging and failure fixtures`
6. `feat(m4): add autostart/single-instance recovery and diagnostics`
7. `docs(m4): record native Windows package and release acceptance`

## 19. Codex 完成报告格式

```md
## M4 完成报告
- 范围：设置、窗口策略、生命周期、日志、打包中实际完成的模块。
- 迁移/恢复：settings schema、默认值、失败回退与用户数据保护证据。
- TDD：每模块红测、相关绿测、完整回归命令/结果。
- 协议：设置页、托盘、Rust 权威快照的一致性证据。
- Windows 验收：OS、DPI/显示器、安装器路径、安装卸载、生命周期与两小时稳定性结果。
- 发布状态：包版本、签名状态、已知风险、回滚包/步骤。
- 未完成：任何未达到的验收项及其阻塞原因。
```

