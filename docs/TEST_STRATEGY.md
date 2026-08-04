# 测试策略

## 目标

证明规则、协议和可恢复性，并用原生 Windows 验证透明窗口与系统集成。测试不替代体验验收，体验验收也不替代规则测试。

## 测试金字塔

| 层级 | 技术/位置 | 覆盖对象 | 要求 |
| --- | --- | --- | --- |
| Rust 单元 | `src-tauri/src/*` 的 `#[cfg(test)]` 或同模块测试 | 状态迁移、计划、坐标、落点、设置、冷却和 Windows 策略 | 纯函数优先、固定时钟/随机源、mock port |
| TypeScript 单元 | `src/**/*.test.ts` + Vitest | 协议解析、手势识别、计划播放、插值、帧与气泡展示 | 不依赖真实 Tauri 或真实时间 |
| 协议/集成 | Rust 载荷夹具 + TS 解析/桥接测试 | 版本、枚举、序号、命令/事件、降级 | 同一 JSON 夹具由两端消费 |
| Windows 手工 | 原生 Windows 10/11 x64 | 透明、置顶、DPI、前台窗口、全屏、托盘、安装器 | 每个里程碑有可复现步骤和结果记录 |

## TDD 规则

每一个模块依次：失败测试 → 确认红 → 最小实现 → 相关测试绿 → 重构 → 全量回归。测试名称描述可见规则而非实现细节。测试所用时钟、随机数、显示器快照和前台窗口快照必须可注入。

## Windows 适配测试模式

1. 定义小 trait（例如 `EnvironmentPort`），输出 `MonitorSnapshot`、`ForegroundWindowSnapshot`、`WindowRect` 等纯数据。
2. 用 fake/mock 输入把策略写成 Rust 单测：DPI 换算、落脚选择、显示模式、恢复和隐藏优先级。
3. 用很薄的 Win32/Tauri 实现适配 trait；该层只负责调用、转换和错误映射。
4. 在原生 Windows 执行文档化手工场景。WSL/Linux 结果不能替代该步骤。

## 回归门槛

每个模块完成至少运行其新增单测；每个里程碑完成运行：

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

Windows 相关里程碑还要完成相应手工验收。当前工作区若 `src-tauri/target` 缓存残留旧绝对路径，可使用临时 `CARGO_TARGET_DIR` 验证，不能据此修改业务代码。

## 协议夹具要求

在实际增加版本化协议时，在 `tests/fixtures/protocol/`（或里程碑实现时确认的同等目录）维护：合法消息、未知版本、非法字段、过期序号和兼容迁移消息。Rust 端序列化、TypeScript 端解析和事件桥接必须使用同一批语义样本。

## Windows 验收记录

每次验收记录操作系统版本、缩放比例、显示器布局、前台应用、步骤、预期、实际和日志位置。失败不能以“机器差异”关闭，除非已记录可复现环境差异及回滚/降级行为。

