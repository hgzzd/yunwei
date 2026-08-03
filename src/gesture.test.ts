import { describe, expect, it } from "vitest";
import { GestureTracker, type PointerPoint } from "./gesture";

const point = (screenX: number, screenY: number, pointerId = 1): PointerPoint => ({
  pointerId,
  screenX,
  screenY,
});

describe("GestureTracker", () => {
  it("emits a click below the drag threshold", () => {
    const tracker = new GestureTracker(6);
    tracker.pointerDown(point(10, 10));
    expect(tracker.pointerMove(point(13, 14))).toEqual([]);
    expect(tracker.pointerUp(point(13, 14))).toEqual([
      { kind: "click", point: point(13, 14) },
    ]);
  });

  it("emits drag start once, followed by moves and an end", () => {
    const tracker = new GestureTracker(6);
    tracker.pointerDown(point(10, 10));
    expect(tracker.pointerMove(point(16, 10))).toEqual([
      { kind: "dragStart", point: point(10, 10) },
      { kind: "dragMove", point: point(16, 10) },
    ]);
    expect(tracker.pointerMove(point(20, 12))).toEqual([
      { kind: "dragMove", point: point(20, 12) },
    ]);
    expect(tracker.pointerUp(point(21, 12))).toEqual([
      { kind: "dragEnd", point: point(21, 12) },
    ]);
  });

  it("ignores unrelated pointers and closes an active drag on cancel", () => {
    const tracker = new GestureTracker(2);
    tracker.pointerDown(point(1, 1));
    expect(tracker.pointerMove(point(9, 9, 2))).toEqual([]);
    tracker.pointerMove(point(4, 1));
    expect(tracker.pointerCancel(point(4, 1))).toEqual([
      { kind: "dragEnd", point: point(4, 1) },
    ]);
    expect(tracker.isDragging()).toBe(false);
  });
});
