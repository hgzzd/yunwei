import { describe, expect, it } from "vitest";
import {
  normalizeBubbleMessage,
  normalizeRenderState,
  normalizeTutorialStep,
  tutorialText,
} from "./pet-model";

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
});
