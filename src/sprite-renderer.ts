import { animationFor, frameAtTime, frameRect } from "./animation";
import type { RenderState, SpriteManifest } from "./pet-model";

export class SpriteRenderer {
  private readonly images = new Map<string, HTMLImageElement>();
  private readonly trimmedFrames = new Map<string, ReturnType<typeof frameRect>>();
  private startTime = performance.now();
  private state: RenderState = { state: "idle", facing: "right" };

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly manifest: SpriteManifest,
  ) {}

  async load(): Promise<boolean> {
    const urls = new Set<string>([this.manifest.imageUrl]);
    for (const animation of Object.values(this.manifest.animations)) {
      if (animation?.imageUrl) urls.add(animation.imageUrl);
    }

    const results = await Promise.all([...urls].map(async (url) => {
      const image = new Image();
      image.decoding = "async";
      const loaded = new Promise<boolean>((resolve) => {
        image.addEventListener("load", () => resolve(true), { once: true });
        image.addEventListener("error", () => resolve(false), { once: true });
      });
      image.src = url;
      if (await loaded) this.images.set(url, image);
      return this.images.has(url);
    }));

    return results.every(Boolean);
  }

  setState(next: RenderState): void {
    if (next.state !== this.state.state) this.startTime = performance.now();
    this.state = next;
  }

  resize(width: number, height: number, pixelRatio = window.devicePixelRatio || 1): void {
    const safeWidth = Math.max(1, Math.round(width * pixelRatio));
    const safeHeight = Math.max(1, Math.round(height * pixelRatio));
    if (this.canvas.width !== safeWidth) this.canvas.width = safeWidth;
    if (this.canvas.height !== safeHeight) this.canvas.height = safeHeight;
  }

  draw(now = performance.now()): void {
    const context = this.canvas.getContext("2d");
    if (!context) return;

    const width = this.canvas.width;
    const height = this.canvas.height;
    context.clearRect(0, 0, width, height);

    const animation = animationFor(this.manifest, this.state.state);
    const imageUrl = animation.imageUrl ?? this.manifest.imageUrl;
    const image = this.images.get(imageUrl) ?? this.images.get(this.manifest.imageUrl);
    if (!image) {
      drawFallbackPet(context, width, height, this.state);
      return;
    }

    const frame = frameAtTime(
      animation,
      now - this.startTime,
      animation.loop ? this.state.frame : undefined,
    );
    const sourceWidth = animation.frameWidth
      ?? (animation.columns ? image.naturalWidth / animation.columns : this.manifest.frameWidth);
    const cell = frameRect(frame, image.naturalWidth, {
      frameWidth: sourceWidth,
      frameHeight: animation.frameHeight ?? image.naturalHeight ?? this.manifest.frameHeight,
    });
    const source = this.trimmedFrame(imageUrl, frame, image, cell);
    const scale = Math.min(width / source.width, height / source.height) * 0.94;
    const destinationWidth = source.width * scale;
    const destinationHeight = source.height * scale;
    const destinationX = (width - destinationWidth) / 2;
    const destinationY = height - destinationHeight;
    const direction = this.state.facing === "left" ? -1 : 1;
    context.save();
    if (direction < 0) {
      context.translate(width, 0);
      context.scale(-1, 1);
    }
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = "high";
    context.drawImage(
      image,
      source.x,
      source.y,
      source.width,
      source.height,
      destinationX,
      destinationY,
      destinationWidth,
      destinationHeight,
    );
    context.restore();
  }

  private trimmedFrame(
    imageUrl: string,
    frame: number,
    image: HTMLImageElement,
    source: ReturnType<typeof frameRect>,
  ): ReturnType<typeof frameRect> {
    const key = `${imageUrl}:${frame}`;
    const cached = this.trimmedFrames.get(key);
    if (cached) return cached;
    const trimmed = trimTransparentPixels(image, source);
    this.trimmedFrames.set(key, trimmed);
    return trimmed;
  }
}

function trimTransparentPixels(
  image: HTMLImageElement,
  source: ReturnType<typeof frameRect>,
): ReturnType<typeof frameRect> {
  const sampleWidth = Math.max(1, Math.ceil(source.width));
  const sampleHeight = Math.max(1, Math.ceil(source.height));
  const sample = document.createElement("canvas");
  sample.width = sampleWidth;
  sample.height = sampleHeight;
  const context = sample.getContext("2d", { willReadFrequently: true });
  if (!context) return source;

  try {
    context.drawImage(
      image,
      source.x,
      source.y,
      source.width,
      source.height,
      0,
      0,
      sampleWidth,
      sampleHeight,
    );
    const pixels = context.getImageData(0, 0, sampleWidth, sampleHeight).data;
    let minX = sampleWidth;
    let minY = sampleHeight;
    let maxX = -1;
    let maxY = -1;
    for (let y = 0; y < sampleHeight; y += 1) {
      for (let x = 0; x < sampleWidth; x += 1) {
        if (pixels[(y * sampleWidth + x) * 4 + 3] <= 8) continue;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
    }
    if (maxX < minX || maxY < minY) return source;

    const padding = 6;
    minX = Math.max(0, minX - padding);
    minY = Math.max(0, minY - padding);
    maxX = Math.min(sampleWidth - 1, maxX + padding);
    maxY = Math.min(sampleHeight - 1, maxY + padding);
    const scaleX = source.width / sampleWidth;
    const scaleY = source.height / sampleHeight;
    return {
      x: source.x + minX * scaleX,
      y: source.y + minY * scaleY,
      width: (maxX - minX + 1) * scaleX,
      height: (maxY - minY + 1) * scaleY,
    };
  } catch (error) {
    console.warn("[yunwei] 图集透明区裁切失败，将按原始帧绘制。", error);
    return source;
  }
}

function drawFallbackPet(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  state: RenderState,
): void {
  const direction = state.facing === "left" ? -1 : 1;
  const unit = Math.min(width, height) / 256;
  const xOffset = (width - 256 * unit) / 2;
  const yOffset = height - 256 * unit;
  context.save();
  context.translate(xOffset + (direction < 0 ? 256 * unit : 0), yOffset);
  context.scale(direction * unit, unit);

  context.fillStyle = "#80d5f4";
  context.beginPath();
  context.arc(190, 132, 45, 0, Math.PI * 2);
  context.arc(220, 112, 31, 0, Math.PI * 2);
  context.arc(229, 151, 35, 0, Math.PI * 2);
  context.arc(190, 169, 38, 0, Math.PI * 2);
  context.fill();

  context.fillStyle = "#fffaf0";
  context.strokeStyle = "#35516d";
  context.lineWidth = 5;
  context.beginPath();
  context.ellipse(121, 143, 75, 68, -0.08, 0, Math.PI * 2);
  context.fill();
  context.stroke();

  context.beginPath();
  context.moveTo(69, 104);
  context.lineTo(78, 55);
  context.lineTo(108, 91);
  context.moveTo(126, 83);
  context.lineTo(155, 51);
  context.lineTo(163, 103);
  context.stroke();

  context.fillStyle = "#35516d";
  context.beginPath();
  context.arc(102, 127, 7, 0, Math.PI * 2);
  context.arc(143, 125, 7, 0, Math.PI * 2);
  context.fill();

  context.strokeStyle = "#ff8f82";
  context.lineCap = "round";
  context.lineWidth = 5;
  context.beginPath();
  context.arc(123, 143, 12, 0.2, Math.PI - 0.2);
  context.stroke();

  context.fillStyle = "#ff8f82";
  context.beginPath();
  context.arc(78, 145, 8, 0, Math.PI * 2);
  context.arc(164, 143, 8, 0, Math.PI * 2);
  context.fill();

  context.restore();
}
