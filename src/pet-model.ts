export const PET_STATES = [
  "idle",
  "walking",
  "running",
  "sitting",
  "sleeping",
  "stretching",
  "tumbling",
  "dragged",
] as const;

export type PetState = (typeof PET_STATES)[number];
export type Facing = "left" | "right";
export type WindowKind = "pet" | "bubble";

export interface AnimationSpec {
  frames: readonly number[];
  fps: number;
  loop: boolean;
  imageUrl?: string;
  columns?: number;
  frameWidth?: number;
  frameHeight?: number;
}

export interface SpriteManifest {
  imageUrl: string;
  frameWidth: number;
  frameHeight: number;
  animations: Record<PetState, AnimationSpec>;
}

export interface RenderState {
  state: PetState;
  facing: Facing;
  frame?: number;
}

export interface PetSettings {
  schemaVersion: number;
  scale: number | "small" | "medium" | "large";
  soundEnabled: boolean;
  autostartEnabled: boolean;
  monitorId: string | null;
  normalizedX: number;
  tutorialStep: number;
}

export interface BubbleMessage {
  text: string;
  visible: boolean;
  kind: "speech" | "tutorial";
  durationMs: number;
}

const STATE_ALIASES: Readonly<Record<string, PetState>> = {
  walk: "walking",
  run: "running",
  sit: "sitting",
  sleep: "sleeping",
  stretch: "stretching",
  tumble: "tumbling",
  dragging: "dragged",
};

export function normalizePetState(value: unknown): PetState {
  if (typeof value !== "string") return "idle";
  if ((PET_STATES as readonly string[]).includes(value)) return value as PetState;
  return STATE_ALIASES[value] ?? "idle";
}

export function normalizeFacing(value: unknown): Facing {
  return value === "left" ? "left" : "right";
}

export function normalizeRenderState(payload: unknown): RenderState {
  if (typeof payload === "string") {
    return { state: normalizePetState(payload), facing: "right" };
  }

  if (!payload || typeof payload !== "object") {
    return { state: "idle", facing: "right" };
  }

  const record = payload as Record<string, unknown>;
  const frame = typeof record.frame === "number" && record.frame >= 0
    ? Math.floor(record.frame)
    : undefined;

  return {
    state: normalizePetState(record.state),
    facing: normalizeFacing(record.facing),
    ...(frame === undefined ? {} : { frame }),
  };
}

export function normalizeBubbleMessage(payload: unknown): BubbleMessage | null {
  if (typeof payload === "string") {
    return payload.trim()
      ? { text: payload.trim(), visible: true, kind: "speech", durationMs: 4_000 }
      : null;
  }

  if (!payload || typeof payload !== "object") return null;
  const record = payload as Record<string, unknown>;
  const text = typeof record.text === "string" ? record.text.trim() : "";
  const visible = record.visible !== false && text.length > 0;
  const durationMs = typeof record.durationMs === "number"
    ? Math.max(0, record.durationMs)
    : 4_000;

  return {
    text,
    visible,
    kind: record.kind === "tutorial" ? "tutorial" : "speech",
    durationMs,
  };
}

export function tutorialText(step: number): string | null {
  return ["点我一下？", "还能拖我走！", "右键能管住我。"][step] ?? null;
}

export function normalizeTutorialStep(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.min(3, Math.floor(value)))
    : 0;
}
