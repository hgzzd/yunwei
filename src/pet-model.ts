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

export const PROTOCOL_VERSION = 2 as const;

export type PetState = (typeof PET_STATES)[number];
export type Facing = "left" | "right";
export type WindowKind = "pet" | "bubble";
export const PRESENTATION_PHASES = ["idleLoop", "walkCycle", "jumpPrepare", "jumpAscend", "jumpApex", "jumpDescend", "landCompress", "landRecover", "dragVisual"] as const;
export type PresentationPhase = (typeof PRESENTATION_PHASES)[number];
export type MotionKind = "idle" | "walk" | "jump" | "landing" | "drag";
export const BEHAVIOR_STATES = ["idle", "walking", "jumping", "landing", "dragged"] as const;
export type BehaviorState = (typeof BEHAVIOR_STATES)[number];
export type FootingSource = "desktopWorkArea" | "foregroundWindowTop";
export type DisplayMode = "aboveNormalWindows" | "desktopOnly";
export type VisibilityReason = "fullscreen" | "specifiedApp" | "desktopOnlyForeground" | "monitorUnavailable";

export interface WorldPoint { monitorId: string; xLogical: number; yLogical: number; }
export interface MotionArc { apex: WorldPoint; startOffsetMs: number; endOffsetMs: number; }
export interface PhaseSlice { phase: PresentationPhase; startOffsetMs: number; durationMs: number; }
export interface MotionPlan {
  protocolVersion: 2; sequence: number; id: number; kind: MotionKind; startedAtMs: number;
  durationMs: number; from: WorldPoint; to: WorldPoint; arc?: MotionArc; facing: Facing; phaseSchedule: PhaseSlice[];
}
export interface Footing {
  id: string; monitorId: string; topYLogical: number; minXLogical: number; maxXLogical: number; source: FootingSource;
}
export interface RuntimeSnapshot {
  protocolVersion: 2; sequence: number; behavior: BehaviorState; position: WorldPoint; footing: Footing; activePlan?: MotionPlan;
  displayMode: DisplayMode; manuallyHidden: boolean; visibilityReason: VisibilityReason | null;
}
export type InputObservation =
  | { kind: "singleClick" }
  | { kind: "dragStarted"; pointerXPhysical: number; pointerYPhysical: number }
  | { kind: "dragMoved"; pointerXPhysical: number; pointerYPhysical: number }
  | { kind: "dragEnded"; pointerXPhysical: number; pointerYPhysical: number };
export interface AnimationObservation { protocolVersion: 2; planId: number; phase: PresentationPhase; }

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

export interface TutorialBubbleDirective {
  protocolVersion: 2;
  sequence: number;
  id: number;
  visible: boolean;
  text: string | null;
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

export function parseTutorialBubbleDirective(value: unknown): TutorialBubbleDirective | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  if (v.protocolVersion !== PROTOCOL_VERSION || !positiveInteger(v.sequence) || !positiveInteger(v.id)
    || typeof v.visible !== "boolean" || (v.text !== null && typeof v.text !== "string")) return null;
  if (!hasOnlyKeys(v, ["protocolVersion", "sequence", "id", "visible", "text"])) return null;
  const text = typeof v.text === "string" ? v.text.trim() : null;
  if ((v.visible && !text) || (!v.visible && text !== null)) return null;
  return { protocolVersion: PROTOCOL_VERSION, sequence: v.sequence, id: v.id, visible: v.visible, text };
}

export function parseMotionPlan(value: unknown): MotionPlan | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  if (v.protocolVersion !== PROTOCOL_VERSION || !positiveInteger(v.sequence) || !positiveInteger(v.id)
    || !["idle", "walk", "jump", "landing", "drag"].includes(String(v.kind))
    || !finiteNonNegative(v.startedAtMs) || !positiveInteger(v.durationMs) || !isPoint(v.from) || !isPoint(v.to)
    || (v.from as WorldPoint).monitorId !== (v.to as WorldPoint).monitorId || !Array.isArray(v.phaseSchedule)
    || (v.facing !== "left" && v.facing !== "right")) return null;
  if (!hasOnlyKeys(v, ["protocolVersion", "sequence", "id", "kind", "startedAtMs", "durationMs", "from", "to", "arc", "facing", "phaseSchedule"])) return null;
  const phaseSchedule = v.phaseSchedule.map(parsePhase);
  if (phaseSchedule.some((phase) => !phase) || !contiguous(phaseSchedule as PhaseSlice[], v.durationMs as number)) return null;
  const arc = v.arc === undefined ? undefined : parseArc(v.arc, (v.from as WorldPoint).monitorId, v.durationMs as number);
  if (v.arc !== undefined && !arc) return null;
  return { protocolVersion: PROTOCOL_VERSION, sequence: v.sequence as number, id: v.id as number, kind: v.kind as MotionKind,
    startedAtMs: v.startedAtMs as number, durationMs: v.durationMs as number, from: v.from as WorldPoint,
    to: v.to as WorldPoint, ...(arc ? { arc } : {}), facing: v.facing, phaseSchedule: phaseSchedule as PhaseSlice[] };
}

export function parseRuntimeSnapshot(value: unknown): RuntimeSnapshot | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  if (v.protocolVersion !== PROTOCOL_VERSION || !positiveInteger(v.sequence) || !(BEHAVIOR_STATES as readonly string[]).includes(String(v.behavior))
    || !isPoint(v.position) || !isFooting(v.footing) || v.position.monitorId !== v.footing.monitorId
    || (v.displayMode !== "aboveNormalWindows" && v.displayMode !== "desktopOnly")
    || typeof v.manuallyHidden !== "boolean" || !isVisibilityReason(v.visibilityReason)) return null;
  if (!hasOnlyKeys(v, ["protocolVersion", "sequence", "behavior", "position", "footing", "activePlan", "displayMode", "manuallyHidden", "visibilityReason"])) return null;
  const plan = v.activePlan === undefined ? undefined : parseMotionPlan(v.activePlan);
  if (v.activePlan !== undefined && (!plan || plan.sequence >= v.sequence || plan.from.monitorId !== v.position.monitorId || plan.to.monitorId !== v.position.monitorId)) return null;
  return { protocolVersion: PROTOCOL_VERSION, sequence: v.sequence as number, behavior: v.behavior as BehaviorState,
    position: v.position as WorldPoint, footing: v.footing as Footing, displayMode: v.displayMode,
    manuallyHidden: v.manuallyHidden, visibilityReason: v.visibilityReason, ...(plan ? { activePlan: plan } : {}) };
}

export function isRuntimeVisible(snapshot: RuntimeSnapshot | null): boolean {
  return !snapshot || (!snapshot.manuallyHidden && snapshot.visibilityReason === null);
}

export function parseInputObservation(value: unknown): InputObservation | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  if (v.kind === "singleClick") return { kind: "singleClick" };
  return ["dragStarted", "dragMoved", "dragEnded"].includes(String(v.kind)) && Number.isFinite(v.pointerXPhysical) && Number.isFinite(v.pointerYPhysical)
    ? { kind: v.kind as InputObservation["kind"], pointerXPhysical: v.pointerXPhysical as number, pointerYPhysical: v.pointerYPhysical as number } : null;
}

export function parseAnimationObservation(value: unknown): AnimationObservation | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  return v.protocolVersion === PROTOCOL_VERSION && positiveInteger(v.planId) && (PRESENTATION_PHASES as readonly string[]).includes(String(v.phase))
    ? { protocolVersion: PROTOCOL_VERSION, planId: v.planId as number, phase: v.phase as PresentationPhase } : null;
}

function isPoint(value: unknown): value is WorldPoint {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return hasOnlyKeys(v, ["monitorId", "xLogical", "yLogical"])
    && typeof v.monitorId === "string" && v.monitorId.trim().length > 0 && Number.isFinite(v.xLogical) && Number.isFinite(v.yLogical);
}
function isFooting(value: unknown): value is Footing {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return hasOnlyKeys(v, ["id", "monitorId", "topYLogical", "minXLogical", "maxXLogical", "source"])
    && typeof v.id === "string" && v.id.trim().length > 0 && typeof v.monitorId === "string" && v.monitorId.trim().length > 0
    && Number.isFinite(v.topYLogical) && Number.isFinite(v.minXLogical) && Number.isFinite(v.maxXLogical)
    && (v.minXLogical as number) <= (v.maxXLogical as number)
    && (v.source === "desktopWorkArea" || v.source === "foregroundWindowTop");
}
function isVisibilityReason(value: unknown): value is VisibilityReason | null {
  return value === null || ["fullscreen", "specifiedApp", "desktopOnlyForeground", "monitorUnavailable"].includes(String(value));
}
function parsePhase(value: unknown): PhaseSlice | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  return hasOnlyKeys(v, ["phase", "startOffsetMs", "durationMs"])
    && (PRESENTATION_PHASES as readonly string[]).includes(String(v.phase)) && finiteNonNegative(v.startOffsetMs) && positiveInteger(v.durationMs)
    ? { phase: v.phase as PresentationPhase, startOffsetMs: v.startOffsetMs as number, durationMs: v.durationMs as number } : null;
}
function parseArc(value: unknown, monitorId: string, durationMs: number): MotionArc | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  return hasOnlyKeys(v, ["apex", "startOffsetMs", "endOffsetMs"])
    && isPoint(v.apex) && v.apex.monitorId === monitorId && finiteNonNegative(v.startOffsetMs) && finiteNonNegative(v.endOffsetMs)
    && (v.startOffsetMs as number) < (v.endOffsetMs as number) && (v.endOffsetMs as number) <= durationMs
    ? { apex: v.apex, startOffsetMs: v.startOffsetMs as number, endOffsetMs: v.endOffsetMs as number } : null;
}
function positiveInteger(value: unknown): value is number { return typeof value === "number" && Number.isInteger(value) && value > 0; }
function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key));
}
function finiteNonNegative(value: unknown): value is number { return typeof value === "number" && Number.isFinite(value) && value >= 0; }
function contiguous(phases: PhaseSlice[], durationMs: number): boolean {
  let next = 0;
  for (const phase of phases) { if (phase.startOffsetMs !== next) return false; next += phase.durationMs; }
  return phases.length > 0 && next === durationMs;
}
