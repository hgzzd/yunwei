import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const readJson = async (file) =>
  JSON.parse(await readFile(resolve(root, file), "utf8"));

const packageJson = await readJson("package.json");
const tauri = await readJson("src-tauri/tauri.conf.json");
const failures = [];

const assert = (condition, message) => {
  if (!condition) failures.push(message);
};

assert(packageJson.name === "yunweishou-desktop", "package name 必须是 yunweishou-desktop");
for (const script of [
  "build",
  "typecheck",
  "test",
  "assets:check",
  "config:check",
  "check",
  "tauri:dev",
  "tauri:build:windows",
]) {
  assert(Boolean(packageJson.scripts?.[script]), `缺少 npm script: ${script}`);
}
assert(
  packageJson.scripts?.["tauri:build:windows"]?.includes("x86_64-pc-windows-msvc") &&
    packageJson.scripts["tauri:build:windows"].includes("--bundles nsis"),
  "Windows 构建脚本必须固定 x64 MSVC target 和 NSIS bundle",
);

assert(tauri.productName === "云尾兽", "Tauri productName 必须是云尾兽");
assert(tauri.identifier === "com.yunweishou.desktop", "应用 identifier 不符合约定");
assert(
  Array.isArray(tauri.bundle?.targets) && tauri.bundle.targets.length === 1 && tauri.bundle.targets[0] === "nsis",
  "bundle targets 必须只包含 nsis",
);
assert(
  tauri.bundle?.windows?.webviewInstallMode?.type === "downloadBootstrapper",
  "WebView2 安装模式必须是 downloadBootstrapper",
);
assert(
  tauri.bundle?.windows?.nsis?.installMode === "currentUser",
  "NSIS 必须使用 currentUser 安装模式",
);

const windows = new Map((tauri.app?.windows ?? []).map((window) => [window.label, window]));
assert(windows.size === 2, "应用必须且只能声明 pet、bubble 两个窗口");

for (const label of ["pet", "bubble"]) {
  const window = windows.get(label);
  assert(Boolean(window), `缺少 ${label} 窗口`);
  if (!window) continue;
  assert(window.transparent === true, `${label} 窗口必须透明`);
  assert(window.decorations === false, `${label} 窗口必须无边框`);
  assert(window.alwaysOnTop === true, `${label} 窗口必须置顶`);
  assert(window.skipTaskbar === true, `${label} 窗口必须跳过任务栏`);
  assert(window.resizable === false, `${label} 窗口不可缩放`);
}

const bubble = windows.get("bubble");
if (bubble) {
  assert(bubble.width === 260 && bubble.height === 96, "bubble 窗口必须与 Rust 定位常量保持 260×96");
  assert(bubble.visible === false, "bubble 窗口启动时必须隐藏");
  assert(bubble.focusable === false, "bubble 窗口不得抢占键盘焦点");
}

if (failures.length > 0) {
  console.error("配置校验失败：");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log("配置校验通过：产品信息、双透明窗口和 Windows NSIS 配置符合约定。");
}
