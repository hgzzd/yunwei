import {
  PET_STATES,
  type AnimationSpec,
  type PetState,
  type SpriteManifest,
} from "./pet-model";

export async function fetchSpriteManifest(url: string): Promise<SpriteManifest | null> {
  try {
    const response = await fetch(url, { cache: "no-cache" });
    if (!response.ok) return null;
    const baseUrl = new URL(".", new URL(url, window.location.href)).href;
    return parseSpriteManifest(await response.json(), baseUrl);
  } catch (error) {
    console.warn("[yunwei] 无法读取正式图集 manifest，将使用内置配置。", error);
    return null;
  }
}

export function parseSpriteManifest(payload: unknown, baseUrl: string): SpriteManifest | null {
  if (!isRecord(payload) || !isPositiveNumber(payload.frameWidth) || !isPositiveNumber(payload.frameHeight)) {
    return null;
  }
  if (typeof payload.imageUrl !== "string" || !isRecord(payload.animations)) return null;

  const animations = {} as Record<PetState, AnimationSpec>;
  for (const state of PET_STATES) {
    const animation = parseAnimation(payload.animations[state], baseUrl);
    if (!animation) return null;
    animations[state] = animation;
  }

  return {
    imageUrl: resolveAssetUrl(payload.imageUrl, baseUrl),
    frameWidth: payload.frameWidth,
    frameHeight: payload.frameHeight,
    animations,
  };
}

function parseAnimation(payload: unknown, baseUrl: string): AnimationSpec | null {
  if (!isRecord(payload) || !Array.isArray(payload.frames)) return null;
  const frames = payload.frames.filter(
    (value): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0,
  );
  if (frames.length !== payload.frames.length || frames.length === 0) return null;
  if (!isPositiveNumber(payload.fps) || typeof payload.loop !== "boolean") return null;
  if (payload.columns !== undefined && !isPositiveInteger(payload.columns)) return null;
  if (payload.frameWidth !== undefined && !isPositiveNumber(payload.frameWidth)) return null;
  if (payload.frameHeight !== undefined && !isPositiveNumber(payload.frameHeight)) return null;
  if (payload.imageUrl !== undefined && typeof payload.imageUrl !== "string") return null;

  return {
    frames,
    fps: payload.fps,
    loop: payload.loop,
    ...(typeof payload.imageUrl === "string"
      ? { imageUrl: resolveAssetUrl(payload.imageUrl, baseUrl) }
      : {}),
    ...(typeof payload.columns === "number" ? { columns: payload.columns } : {}),
    ...(typeof payload.frameWidth === "number" ? { frameWidth: payload.frameWidth } : {}),
    ...(typeof payload.frameHeight === "number" ? { frameHeight: payload.frameHeight } : {}),
  };
}

function resolveAssetUrl(value: string, baseUrl: string): string {
  if (value.startsWith("assets/")) {
    return new URL(`/${value}`, baseUrl).href;
  }
  return new URL(value, baseUrl).href;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isPositiveNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function isPositiveInteger(value: unknown): value is number {
  return isPositiveNumber(value) && Number.isInteger(value);
}
