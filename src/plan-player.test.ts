import { describe, expect, it } from "vitest";
import { PlanPlayer } from "./plan-player";
import fixture from "../tests/fixtures/protocol/m1-jump.json";

const jump = {
  protocolVersion: 2,
  sequence: 2,
  id: 9,
  kind: "jump",
  startedAtMs: 1_000,
  durationMs: 1_500,
  from: { monitorId: "primary", xLogical: 100, yLogical: 420 },
  to: { monitorId: "primary", xLogical: 196, yLogical: 420 },
  arc: { apex: { monitorId: "primary", xLogical: 148, yLogical: 348 }, startOffsetMs: 220, endOffsetMs: 940 },
  facing: "right",
  phaseSchedule: [
    { phase: "jumpPrepare", startOffsetMs: 0, durationMs: 220 },
    { phase: "jumpAscend", startOffsetMs: 220, durationMs: 180 },
    { phase: "jumpApex", startOffsetMs: 400, durationMs: 220 },
    { phase: "jumpDescend", startOffsetMs: 620, durationMs: 320 },
    { phase: "landCompress", startOffsetMs: 940, durationMs: 240 },
    { phase: "landRecover", startOffsetMs: 1_180, durationMs: 320 },
  ],
};

describe("PlanPlayer", () => {
  it("interpolates a Rust jump plan and rejects an older replacement", () => {
    const player = new PlanPlayer();
    expect(player.acceptMotionPlan(jump)).toBe(true);
    expect(player.sample(1_510)?.phase).toBe("jumpApex");
    expect(player.sample(1_510)?.position.yLogical).toBeLessThan(420);
    expect(player.acceptMotionPlan({ ...jump, sequence: 1, id: 8 })).toBe(false);
  });

  it("consumes the shared Rust protocol fixture", () => {
    const player = new PlanPlayer();
    expect(player.acceptMotionPlan(fixture)).toBe(true);
  });

  it("replaces a walk with a newer drag snapshot and never accepts an old snapshot", () => {
    const player = new PlanPlayer();
    expect(player.acceptMotionPlan({ ...jump, kind: "walk", sequence: 3, id: 3, phaseSchedule: [{ phase: "walkCycle", startOffsetMs: 0, durationMs: 1_500 }] })).toBe(true);
    const drag = { protocolVersion: 2, sequence: 6, behavior: "dragged",
      position: { monitorId: "primary", xLogical: 130, yLogical: 300 },
      footing: { id: "desktop", monitorId: "primary", topYLogical: 420, minXLogical: 0, maxXLogical: 500, source: "desktopWorkArea" },
      displayMode: "aboveNormalWindows", manuallyHidden: false, visibilityReason: null,
      activePlan: { ...jump, sequence: 5, id: 4, kind: "drag", from: { monitorId: "primary", xLogical: 130, yLogical: 300 }, to: { monitorId: "primary", xLogical: 130, yLogical: 300 }, arc: undefined, durationMs: 60_000, phaseSchedule: [{ phase: "dragVisual", startOffsetMs: 0, durationMs: 60_000 }] },
    };
    expect(player.acceptRuntimeSnapshot(drag)).toBe(true);
    expect(player.sample(1_010)?.phase).toBe("dragVisual");
    expect(player.acceptRuntimeSnapshot({ ...drag, sequence: 2 })).toBe(false);
  });
});
