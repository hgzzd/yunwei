import { describe, expect, it } from "vitest";
import { RuntimeSnapshotReceiver } from "./runtime-snapshot-receiver";

const visibleSnapshot = {
  protocolVersion: 2,
  sequence: 8,
  behavior: "idle",
  position: { monitorId: "primary", xLogical: 100, yLogical: 420 },
  footing: {
    id: "desktop",
    monitorId: "primary",
    topYLogical: 420,
    minXLogical: 0,
    maxXLogical: 500,
    source: "desktopWorkArea",
  },
  displayMode: "aboveNormalWindows",
  manuallyHidden: false,
  visibilityReason: null,
};

describe("RuntimeSnapshotReceiver", () => {
  it("keeps the newer visible snapshot when a late hidden snapshot arrives", () => {
    const receiver = new RuntimeSnapshotReceiver();

    expect(receiver.accept({ ...visibleSnapshot, sequence: 7, visibilityReason: "fullscreen" })?.visible).toBe(false);
    expect(receiver.accept(visibleSnapshot)?.visible).toBe(true);
    expect(receiver.accept({ ...visibleSnapshot, sequence: 7, visibilityReason: "fullscreen" })).toBeNull();
    expect(receiver.current()?.visible).toBe(true);
  });
});
