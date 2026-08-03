import { readdir, readFile, stat } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = resolve(root, "public/assets/sprites/manifest.json");
const spriteRoot = resolve(root, "public/assets/sprites");
const audioRoot = resolve(root, "public/assets/audio");
const iconRoot = resolve(root, "src-tauri/icons");
const failures = [];
const pngDimensions = new Map();
const expectedFrameCounts = {
  idle: 4,
  walking: 8,
  running: 6,
  sitting: 4,
  sleeping: 6,
  stretching: 6,
  tumbling: 8,
  dragged: 2,
};

const fail = (message) => failures.push(message);

const readRequired = async (file, label) => {
  try {
    const info = await stat(file);
    if (!info.isFile() || info.size === 0) throw new Error("文件为空或不是普通文件");
    return await readFile(file);
  } catch (error) {
    fail(`${label}: ${error.message}`);
    return null;
  }
};

const parsePng = (buffer, label) => {
  if (!buffer || buffer.length < 33) {
    fail(`${label}: PNG 数据不完整`);
    return null;
  }
  const signature = "89504e470d0a1a0a";
  if (buffer.subarray(0, 8).toString("hex") !== signature) {
    fail(`${label}: 不是有效的 PNG 文件`);
    return null;
  }
  const width = buffer.readUInt32BE(16);
  const height = buffer.readUInt32BE(20);
  const colorType = buffer[25];
  if (width === 0 || height === 0) fail(`${label}: PNG 尺寸不能为 0`);
  if (![4, 6].includes(colorType)) fail(`${label}: PNG 必须包含 alpha 通道`);
  return { width, height };
};

let manifest = null;
try {
  manifest = JSON.parse(await readFile(manifestPath, "utf8"));
} catch (error) {
  fail(`图集清单 public/assets/sprites/manifest.json: ${error.message}`);
}

const pngReferences = new Set();
const collectPngReferences = (value) => {
  if (typeof value === "string" && extname(value.split(/[?#]/, 1)[0]).toLowerCase() === ".png") {
    pngReferences.add(value);
  } else if (Array.isArray(value)) {
    value.forEach(collectPngReferences);
  } else if (value && typeof value === "object") {
    Object.values(value).forEach(collectPngReferences);
  }
};

if (manifest) {
  collectPngReferences(manifest);
  if (pngReferences.size === 0) fail("图集清单至少要引用一个 PNG 文件");
  if (manifest.frameWidth !== 256 || manifest.frameHeight !== 256) {
    fail("图集清单的 frameWidth/frameHeight 必须是 256×256");
  }

  const animations = manifest.animations;
  for (const [state, expectedCount] of Object.entries(expectedFrameCounts)) {
    if (!animations || !(state in animations)) {
      fail(`图集清单缺少动作: ${state}`);
      continue;
    }
    const frames = animations[state]?.frames;
    if (!Array.isArray(frames) || frames.length !== expectedCount) {
      fail(`动作 ${state} 必须包含 ${expectedCount} 个帧索引`);
    } else if (!frames.every((frame) => Number.isInteger(frame) && frame >= 0)) {
      fail(`动作 ${state} 的帧索引必须是非负整数`);
    }
    if (animations[state]?.fps !== 12) fail(`动作 ${state} 必须以 12fps 播放`);
    if (typeof animations[state]?.loop !== "boolean") fail(`动作 ${state} 必须明确 loop 布尔值`);
  }
}

const resolvePublicAsset = (reference) => {
  const clean = reference.split(/[?#]/, 1)[0].replaceAll("\\", "/");
  if (clean.startsWith("/")) return resolve(root, "public", clean.slice(1));
  if (clean.startsWith("assets/")) return resolve(root, "public", clean);
  return resolve(spriteRoot, clean);
};

for (const reference of pngReferences) {
  const file = resolvePublicAsset(reference);
  const relative = file.startsWith(root) ? file.slice(root.length + 1) : reference;
  if (!file.startsWith(spriteRoot)) {
    fail(`图集 PNG 必须位于 public/assets/sprites: ${reference}`);
    continue;
  }
  const dimensions = parsePng(await readRequired(file, relative), relative);
  if (dimensions) {
    pngDimensions.set(reference, dimensions);
    if (dimensions.width % 256 !== 0 || dimensions.height % 256 !== 0) {
      fail(`${relative}: 图集宽高必须是 256 的整数倍，实际为 ${dimensions.width}×${dimensions.height}`);
    }
  }
}

if (manifest?.animations) {
  for (const [state, animation] of Object.entries(manifest.animations)) {
    if (!animation || typeof animation !== "object") continue;
    const imageUrl = animation.imageUrl ?? manifest.imageUrl;
    const dimensions = pngDimensions.get(imageUrl);
    if (!dimensions) continue;
    const frameWidth = animation.frameWidth ?? manifest.frameWidth;
    const frameHeight = animation.frameHeight ?? manifest.frameHeight;
    if (frameWidth !== 256 || frameHeight !== 256) {
      fail(`动作 ${state} 的有效帧尺寸必须是 256×256`);
      continue;
    }
    const capacity = (dimensions.width / frameWidth) * (dimensions.height / frameHeight);
    if (animation.frames?.some((frame) => frame >= capacity)) {
      fail(`动作 ${state} 的帧索引超出图集容量 ${capacity}`);
    }
  }
}

let wavFiles = [];
try {
  wavFiles = (await readdir(audioRoot)).filter((file) => extname(file).toLowerCase() === ".wav");
} catch (error) {
  fail(`音效目录 public/assets/audio: ${error.message}`);
}
if (wavFiles.length === 0) fail("public/assets/audio 至少需要一个 WAV 音效");
for (const name of wavFiles) {
  const buffer = await readRequired(resolve(audioRoot, name), `public/assets/audio/${name}`);
  if (buffer && (buffer.length < 12 || buffer.toString("ascii", 0, 4) !== "RIFF" || buffer.toString("ascii", 8, 12) !== "WAVE")) {
    fail(`public/assets/audio/${name}: 不是有效的 RIFF/WAVE 文件`);
  }
}

for (const name of ["32x32.png", "128x128.png", "128x128@2x.png"]) {
  parsePng(await readRequired(resolve(iconRoot, name), `src-tauri/icons/${name}`), `src-tauri/icons/${name}`);
}
const ico = await readRequired(resolve(iconRoot, "icon.ico"), "src-tauri/icons/icon.ico");
if (ico && (ico.length < 6 || ico.readUInt16LE(0) !== 0 || ico.readUInt16LE(2) !== 1 || ico.readUInt16LE(4) === 0)) {
  fail("src-tauri/icons/icon.ico: ICO 文件头无效");
}

if (failures.length > 0) {
  console.error("资源校验失败：");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(`资源校验通过：${pngReferences.size} 个图集、${wavFiles.length} 个 WAV 音效及 Windows 图标有效。`);
}
