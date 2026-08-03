import { describe, expect, it } from "vitest";
import { animationFor, frameAtTime, frameRect } from "./animation";
import type { SpriteManifest } from "./pet-model";

const manifest: SpriteManifest = {
  imageUrl: "sprite.png",
  frameWidth: 64,
  frameHeight: 32,
  animations: {
    idle: { frames: [0, 1], fps: 2, loop: true },
    walking: { frames: [2, 3, 4], fps: 10, loop: true },
    stretching: { frames: [5, 6], fps: 2, loop: false },
    running: { frames: [0], fps: 1, loop: true },
    sitting: { frames: [0], fps: 1, loop: true },
    sleeping: { frames: [0], fps: 1, loop: true },
    tumbling: { frames: [0], fps: 1, loop: false },
    dragged: { frames: [0], fps: 1, loop: true },
  },
};

describe("sprite animation", () => {
  it("advances and loops frame sequences", () => {
    expect(frameAtTime(manifest.animations.idle, 0)).toBe(0);
    expect(frameAtTime(manifest.animations.idle, 500)).toBe(1);
    expect(frameAtTime(manifest.animations.idle, 1_000)).toBe(0);
  });

  it("clamps non-looping sequences and explicit frames", () => {
    const stretch = manifest.animations.stretching!;
    expect(frameAtTime(stretch, 50_000)).toBe(6);
    expect(frameAtTime(stretch, 0, 99)).toBe(6);
    expect(frameAtTime(stretch, 0, -1)).toBe(5);
  });

  it("wraps an explicit native frame for a shorter looping sheet", () => {
    expect(frameAtTime(manifest.animations.idle, 0, 5)).toBe(1);
  });

  it("falls back to idle for a missing state", () => {
    const manifestWithoutSleep = {
      ...manifest,
      animations: { ...manifest.animations, sleeping: undefined },
    } as unknown as SpriteManifest;
    expect(animationFor(manifestWithoutSleep, "sleeping")).toBe(manifest.animations.idle);
  });

  it("maps a frame index onto a multi-row atlas", () => {
    expect(frameRect(6, 256, manifest)).toEqual({ x: 128, y: 32, width: 64, height: 32 });
  });
});
