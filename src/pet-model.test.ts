import { describe, expect, it } from "vitest";
import {
  normalizeBubbleMessage,
  parseMotionPlan,
  parseRuntimeSnapshot,
  isRuntimeVisible,
  normalizeRenderState,
  normalizeTutorialStep,
  tutorialText,
} from "./pet-model";
import startupSnapshot from "../tests/fixtures/protocol/m1-startup-snapshot.json";
import invalidFacing from "../tests/fixtures/protocol/m1-invalid-facing.json";
import m2DualMonitor from "../tests/fixtures/protocol/m2-dual-monitor-150.json";

describe("native payload normalization", () => {
  it("accepts Rust state aliases and rejects malformed values", () => {
    expect(normalizeRenderState({ state: "sleeping", facing: "left", frame: 2.8 })).toEqual({
      state: "sleeping",
      facing: "left",
      frame: 2,
    });
    expect(normalizeRenderState({ state: "unknown", facing: "up" })).toEqual({
      state: "idle",
      facing: "right",
    });
  });

  it("normalizes bubble defaults and visibility", () => {
    expect(normalizeBubbleMessage("今天也要摸鱼")).toEqual({
      text: "今天也要摸鱼",
      visible: true,
      kind: "speech",
      durationMs: 4_000,
    });
    expect(normalizeBubbleMessage({ text: "", visible: true })).toEqual({
      text: "",
      visible: false,
      kind: "speech",
      durationMs: 4_000,
    });
  });

  it("keeps tutorial progress inside the supported range", () => {
    expect(normalizeTutorialStep(-2)).toBe(0);
    expect(normalizeTutorialStep(9)).toBe(3);
    expect(tutorialText(0)).toBe("点我一下？");
    expect(tutorialText(3)).toBeNull();
  });

  it("accepts only complete versioned motion plans", () => {
    const plan = {
      protocolVersion: 1, sequence: 1, id: 1, kind: "walk", startedAtMs: 0, durationMs: 100,
      from: { monitorId: "primary", xLogical: 0, yLogical: 10 }, to: { monitorId: "primary", xLogical: 10, yLogical: 10 }, facing: "right",
      phaseSchedule: [{ phase: "walkCycle", startOffsetMs: 0, durationMs: 100 }],
    };
    expect(parseMotionPlan(plan)?.kind).toBe("walk");
    expect(parseMotionPlan({ ...plan, protocolVersion: 2 })).toBeNull();
    expect(parseMotionPlan({ ...plan, phaseSchedule: [{ phase: "bad", startOffsetMs: 0, durationMs: 100 }] })).toBeNull();
  });

  it("accepts complete snapshots and rejects an invalid plan facing", () => {
    const plan = {
      protocolVersion: 1, sequence: 7, id: 7, kind: "walk", startedAtMs: 0, durationMs: 100,
      from: { monitorId: "primary", xLogical: 100, yLogical: 420 }, to: { monitorId: "primary", xLogical: 120, yLogical: 420 }, facing: "right",
      phaseSchedule: [{ phase: "walkCycle", startOffsetMs: 0, durationMs: 100 }],
    };
    const snapshot = {
      protocolVersion: 1, sequence: 8, behavior: "walking",
      position: { monitorId: "primary", xLogical: 110, yLogical: 420 },
      footing: { id: "desktop", monitorId: "primary", topYLogical: 420, minXLogical: 0, maxXLogical: 500, source: "desktopWorkArea" },
      activePlan: plan,
    };
    expect(parseRuntimeSnapshot(snapshot)?.position).toEqual(snapshot.position);
    expect(parseRuntimeSnapshot({ ...snapshot, activePlan: { ...plan, facing: "up" } })).toBeNull();
    expect(parseRuntimeSnapshot(startupSnapshot)).not.toBeNull();
    expect(parseRuntimeSnapshot(invalidFacing)).toBeNull();
    expect(parseRuntimeSnapshot(m2DualMonitor)?.footing.source).toBe("foregroundWindowTop");
  });

  it("M2 defaults legacy snapshots and safely hides unknown environment reasons", () => {
    const legacy = {
      protocolVersion: 1, sequence: 8, behavior: "idle",
      position: { monitorId: "primary", xLogical: 110, yLogical: 420 },
      footing: { id: "top", monitorId: "primary", topYLogical: 420, minXLogical: 0, maxXLogical: 500, source: "foregroundWindowTop" },
    };
    expect(parseRuntimeSnapshot(legacy)).toMatchObject({ displayMode: "aboveNormalWindows", manuallyHidden: false, visibilityReason: null });
    const unknown = parseRuntimeSnapshot({ ...legacy, sequence: 9, visibilityReason: "futureReason" });
    expect(unknown?.visibilityReason).toBe("unknown");
    expect(isRuntimeVisible(unknown ?? null)).toBe(false);
  });
});
