import { describe, expect, it } from "vitest";
import { PET_STATES } from "./pet-model";
import { parseSpriteManifest } from "./sprite-manifest";

function validManifest(): Record<string, unknown> {
  return {
    imageUrl: "fallback.png",
    frameWidth: 256,
    frameHeight: 256,
    animations: Object.fromEntries(PET_STATES.map((state) => [state, {
      frames: [0, 1],
      fps: 12,
      loop: state !== "stretching" && state !== "tumbling",
      imageUrl: `${state}.png`,
      columns: 2,
    }])),
  };
}

describe("sprite manifest", () => {
  it("accepts all canonical states and resolves relative asset URLs", () => {
    const manifest = parseSpriteManifest(validManifest(), "https://pet.local/assets/sprites/");
    expect(manifest?.imageUrl).toBe("https://pet.local/assets/sprites/fallback.png");
    expect(manifest?.animations.walking.imageUrl).toBe("https://pet.local/assets/sprites/walking.png");
    expect(manifest?.animations.tumbling.loop).toBe(false);
  });

  it("treats assets-prefixed URLs as application-root relative", () => {
    const payload = validManifest();
    (payload.animations as Record<string, Record<string, unknown>>).idle.imageUrl = "assets/sprites/idle.png";
    const manifest = parseSpriteManifest(payload, "https://pet.local/assets/sprites/");
    expect(manifest?.animations.idle.imageUrl).toBe("https://pet.local/assets/sprites/idle.png");
  });

  it("rejects missing states and invalid frame lists", () => {
    const missing = validManifest();
    delete (missing.animations as Record<string, unknown>).dragged;
    expect(parseSpriteManifest(missing, "https://pet.local/assets/sprites/")).toBeNull();

    const badFrames = validManifest();
    (badFrames.animations as Record<string, Record<string, unknown>>).idle.frames = [0, -1];
    expect(parseSpriteManifest(badFrames, "https://pet.local/assets/sprites/")).toBeNull();
  });
});
