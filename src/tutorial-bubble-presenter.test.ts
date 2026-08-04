import { describe, expect, it } from "vitest";
import { TutorialBubblePresenter } from "./tutorial-bubble-presenter";

const snapshot = (sequence: number, manuallyHidden = false) => ({
  protocolVersion: 2, sequence, behavior: "idle", position: { monitorId: "primary", xLogical: 10, yLogical: 20 },
  footing: { id: "desktop", monitorId: "primary", topYLogical: 20, minXLogical: 0, maxXLogical: 100, source: "desktopWorkArea" },
  displayMode: "aboveNormalWindows", manuallyHidden, visibilityReason: null,
});

describe("TutorialBubblePresenter", () => {
  it("replaces directives by sequence and ignores stale replacements", () => {
    const presenter = new TutorialBubblePresenter();
    expect(presenter.acceptDirective({ protocolVersion: 2, sequence: 2, id: 2, visible: true, text: "第二句" }))
      .toEqual({ visible: true, text: "第二句", id: 2 });
    expect(presenter.acceptDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: true, text: "旧句" })).toBeNull();
    expect(presenter.current()).toEqual({ visible: true, text: "第二句", id: 2 });
  });

  it("uses the same sequenced snapshots to hide and recover without a local timer", () => {
    const presenter = new TutorialBubblePresenter();
    presenter.acceptDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: true, text: "点我一下？" });
    expect(presenter.acceptRuntimeSnapshot(snapshot(4, true))).toEqual({ visible: false, text: "", id: null });
    expect(presenter.acceptRuntimeSnapshot(snapshot(3, false))).toBeNull();
    expect(presenter.current()).toEqual({ visible: false, text: "", id: null });
    expect(presenter.acceptRuntimeSnapshot(snapshot(5, false))).toEqual({ visible: true, text: "点我一下？", id: 1 });
  });

  it("honors a late-fetched hidden startup snapshot before rendering", () => {
    const presenter = new TutorialBubblePresenter();
    presenter.acceptDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: true, text: "点我一下？" });
    presenter.acceptRuntimeSnapshot(snapshot(1, true));
    expect(presenter.current()).toEqual({ visible: false, text: "", id: null });
  });

  it("rejects v1 and inconsistent Rust directives", () => {
    const presenter = new TutorialBubblePresenter();
    expect(presenter.acceptDirective({ protocolVersion: 1, sequence: 1, id: 1, visible: true, text: "旧协议" })).toBeNull();
    expect(presenter.acceptDirective({ protocolVersion: 2, sequence: 1, id: 1, visible: false, text: "错误" })).toBeNull();
  });
});
