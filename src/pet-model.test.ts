import { describe, expect, it } from "vitest";
import {
  parseTutorialBubbleDirective,
  parseMotionPlan,
  parseRuntimeSnapshot,
  isRuntimeVisible,
  normalizeRenderState,
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

  it("accepts only complete Rust-issued tutorial directives", () => {
    expect(parseTutorialBubbleDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: true, text: "点我一下？" }))
      .toEqual({ protocolVersion: 2, sequence: 1, id: 1, visible: true, text: "点我一下？" });
    expect(parseTutorialBubbleDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: false, text: "本地文本" })).toBeNull();
    expect(parseTutorialBubbleDirective({ protocolVersion: 1, sequence: 1, id: 1, visible: true, text: "旧协议" })).toBeNull();
  });

  it("accepts only complete versioned motion plans", () => {
    const plan = {
      protocolVersion: 2, sequence: 1, id: 1, kind: "walk", startedAtMs: 0, durationMs: 100,
      from: { monitorId: "primary", xLogical: 0, yLogical: 10 }, to: { monitorId: "primary", xLogical: 10, yLogical: 10 }, facing: "right",
      phaseSchedule: [{ phase: "walkCycle", startOffsetMs: 0, durationMs: 100 }],
    };
    expect(parseMotionPlan(plan)?.kind).toBe("walk");
    expect(parseMotionPlan({ ...plan, protocolVersion: 1 })).toBeNull();
    expect(parseMotionPlan({ ...plan, localFallback: true })).toBeNull();
    expect(parseMotionPlan({ ...plan, phaseSchedule: [{ phase: "bad", startOffsetMs: 0, durationMs: 100 }] })).toBeNull();
  });

  it("accepts complete snapshots and rejects an invalid plan facing", () => {
    const plan = {
      protocolVersion: 2, sequence: 7, id: 7, kind: "walk", startedAtMs: 0, durationMs: 100,
      from: { monitorId: "primary", xLogical: 100, yLogical: 420 }, to: { monitorId: "primary", xLogical: 120, yLogical: 420 }, facing: "right",
      phaseSchedule: [{ phase: "walkCycle", startOffsetMs: 0, durationMs: 100 }],
    };
    const snapshot = {
      protocolVersion: 2, sequence: 8, behavior: "walking",
      position: { monitorId: "primary", xLogical: 110, yLogical: 420 },
      footing: { id: "desktop", monitorId: "primary", topYLogical: 420, minXLogical: 0, maxXLogical: 500, source: "desktopWorkArea" },
      displayMode: "aboveNormalWindows",
      manuallyHidden: false,
      visibilityReason: null,
      activePlan: plan,
    };
    expect(parseRuntimeSnapshot(snapshot)?.position).toEqual(snapshot.position);
    expect(parseRuntimeSnapshot({ ...snapshot, frontendVisibility: true })).toBeNull();
    expect(parseRuntimeSnapshot({ ...snapshot, activePlan: { ...plan, facing: "up" } })).toBeNull();
    expect(parseRuntimeSnapshot(startupSnapshot)).not.toBeNull();
    expect(parseRuntimeSnapshot(invalidFacing)).toBeNull();
    expect(parseRuntimeSnapshot(m2DualMonitor)?.footing.source).toBe("foregroundWindowTop");
  });

  it("rejects legacy and unknown-environment snapshots", () => {
    const legacy = {
      protocolVersion: 1, sequence: 8, behavior: "idle",
      position: { monitorId: "primary", xLogical: 110, yLogical: 420 },
      footing: { id: "top", monitorId: "primary", topYLogical: 420, minXLogical: 0, maxXLogical: 500, source: "foregroundWindowTop" },
    };
    expect(parseRuntimeSnapshot(legacy)).toBeNull();
    const unknown = { ...legacy, protocolVersion: 2, displayMode: "aboveNormalWindows", manuallyHidden: false, visibilityReason: "futureReason" };
    expect(parseRuntimeSnapshot(unknown)).toBeNull();
    expect(isRuntimeVisible(null)).toBe(true);
  });
});
