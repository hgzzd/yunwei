import type { AnimationSpec, PetState, SpriteManifest } from "./pet-model";

export interface FrameRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function animationFor(manifest: SpriteManifest, state: PetState): AnimationSpec {
  const candidate = manifest.animations[state];
  return candidate && candidate.frames.length > 0 ? candidate : manifest.animations.idle;
}

export function frameAtTime(
  animation: AnimationSpec,
  elapsedMs: number,
  explicitFrame?: number,
): number {
  if (animation.frames.length === 0) return 0;
  if (explicitFrame !== undefined) {
    const safeFrame = Math.max(0, explicitFrame);
    const position = animation.loop
      ? safeFrame % animation.frames.length
      : Math.min(safeFrame, animation.frames.length - 1);
    return animation.frames[position];
  }

  const elapsedFrames = Math.floor(Math.max(0, elapsedMs) * Math.max(0, animation.fps) / 1_000);
  const position = animation.loop
    ? elapsedFrames % animation.frames.length
    : Math.min(elapsedFrames, animation.frames.length - 1);
  return animation.frames[position];
}

export function frameRect(
  frame: number,
  imageWidth: number,
  manifest: Pick<SpriteManifest, "frameWidth" | "frameHeight">,
): FrameRect {
  const columns = Math.max(1, Math.floor(imageWidth / manifest.frameWidth));
  const safeFrame = Math.max(0, Math.floor(frame));
  return {
    x: (safeFrame % columns) * manifest.frameWidth,
    y: Math.floor(safeFrame / columns) * manifest.frameHeight,
    width: manifest.frameWidth,
    height: manifest.frameHeight,
  };
}
