import placeholderUrl from "./assets/yunwei-placeholder.svg";
import { GestureTracker, type GestureAction, type PointerPoint } from "./gesture";
import {
  normalizeBubbleMessage,
  normalizeRenderState,
  normalizeTutorialStep,
  tutorialText,
  type BubbleMessage,
  type PetSettings,
  type RenderState,
  type SpriteManifest,
} from "./pet-model";
import { SpriteRenderer } from "./sprite-renderer";
import { fetchSpriteManifest } from "./sprite-manifest";
import { currentWindowKind, safeInvoke, safeListen } from "./tauri-bridge";
import { PetAudioPlayer, SoundGate } from "./pet-audio";

const DEFAULT_MANIFEST: SpriteManifest = {
  imageUrl: placeholderUrl,
  frameWidth: 256,
  frameHeight: 256,
  animations: {
    idle: {
      frames: [0, 1, 2, 3], fps: 12, loop: true,
      imageUrl: "/assets/sprites/idle.png", columns: 4,
    },
    walking: {
      frames: [0, 1, 2, 3], fps: 12, loop: true,
      imageUrl: "/assets/sprites/walk.png", columns: 4,
    },
    running: {
      frames: [0, 1, 2, 3], fps: 12, loop: true,
      imageUrl: "/assets/sprites/run.png", columns: 4,
    },
    sitting: {
      frames: [0, 1, 2, 3], fps: 12, loop: true,
      imageUrl: "/assets/sprites/sit.png", columns: 4,
    },
    sleeping: { frames: [0], fps: 1, loop: true },
    stretching: { frames: [0], fps: 1, loop: false },
    tumbling: { frames: [0], fps: 1, loop: false },
    dragged: { frames: [0], fps: 1, loop: true },
  },
};

export async function startApplication(): Promise<void> {
  const app = document.querySelector<HTMLElement>("#app");
  if (!app) return;

  installGlobalErrorFallback();
  const kind = currentWindowKind();
  document.body.classList.add(`${kind}-window`);
  document.documentElement.dataset.window = kind;

  if (kind === "bubble") {
    await startBubbleWindow(app);
  } else {
    await startPetWindow(app);
  }
}

async function startPetWindow(app: HTMLElement): Promise<void> {
  app.innerHTML = `
    <div class="pet-stage" tabindex="0" role="img" aria-label="云尾兽，正在发呆">
      <canvas class="pet-canvas" aria-hidden="true"></canvas>
    </div>
  `;
  const stage = app.querySelector<HTMLElement>(".pet-stage");
  const canvas = app.querySelector<HTMLCanvasElement>(".pet-canvas");
  if (!stage || !canvas) return;

  const manifest = await fetchSpriteManifest("/assets/sprites/manifest.json")
    ?? DEFAULT_MANIFEST;
  const renderer = new SpriteRenderer(canvas, manifest);
  const assetLoaded = await renderer.load();
  if (!assetLoaded) announce("角色资源加载失败，已启用简化外观。");

  let animationHandle = 0;
  let visible = true;
  const resize = (): void => {
    const bounds = stage.getBoundingClientRect();
    renderer.resize(bounds.width, bounds.height);
  };
  resize();
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(stage);

  const animate = (now: number): void => {
    if (visible) renderer.draw(now);
    animationHandle = window.requestAnimationFrame(animate);
  };
  animationHandle = window.requestAnimationFrame(animate);

  const gesture = new GestureTracker(6);
  const soundGate = new SoundGate();
  const audio = new PetAudioPlayer();
  let tutorialStep = 0;
  let pendingDragMove: PointerPoint | null = null;
  let dragFrame = 0;

  const applySoundSetting = (enabled: boolean): void => {
    soundGate.setEnabled(enabled);
    if (!enabled) audio.stopAll();
  };

  const flushDragMove = (): void => {
    dragFrame = 0;
    if (!pendingDragMove) return;
    const point = pendingDragMove;
    pendingDragMove = null;
    void safeInvoke("drag_pet", { pointerX: point.screenX, pointerY: point.screenY });
  };

  const dispatch = (actions: readonly GestureAction[]): void => {
    for (const action of actions) {
      const { point } = action;
      if (action.kind === "click") {
        audio.play(soundGate.clicked(performance.now()));
        void safeInvoke("pet_clicked");
        if (tutorialStep === 0) advanceTutorial(1);
      } else if (action.kind === "dragStart") {
        document.body.classList.add("is-dragging");
        renderer.setState({ state: "dragged", facing: "right" });
        void safeInvoke("begin_drag", { pointerX: point.screenX, pointerY: point.screenY });
      } else if (action.kind === "dragMove") {
        pendingDragMove = point;
        if (!dragFrame) dragFrame = window.requestAnimationFrame(flushDragMove);
      } else {
        document.body.classList.remove("is-dragging");
        pendingDragMove = null;
        if (dragFrame) window.cancelAnimationFrame(dragFrame);
        dragFrame = 0;
        void safeInvoke("end_drag");
        if (tutorialStep === 1) advanceTutorial(2);
      }
    }
  };

  const advanceTutorial = (step: number): void => {
    tutorialStep = step;
    void safeInvoke("tutorial_advanced", { step });
  };

  stage.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    stage.setPointerCapture(event.pointerId);
    gesture.pointerDown(pointerPoint(event));
  });
  stage.addEventListener("pointermove", (event) => dispatch(gesture.pointerMove(pointerPoint(event))));
  stage.addEventListener("pointerup", (event) => {
    dispatch(gesture.pointerUp(pointerPoint(event)));
    if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId);
  });
  stage.addEventListener("pointercancel", (event) => dispatch(gesture.pointerCancel(pointerPoint(event))));
  stage.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    void safeInvoke("show_context_menu");
    if (tutorialStep === 2) advanceTutorial(3);
  });
  stage.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      audio.play(soundGate.clicked(performance.now()));
      void safeInvoke("pet_clicked");
      if (tutorialStep === 0) advanceTutorial(1);
    } else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      void safeInvoke("show_context_menu");
      if (tutorialStep === 2) advanceTutorial(3);
    }
  });

  const unlisteners = await Promise.all([
    safeListen<unknown>("pet://state", (payload) => {
      const state = normalizeRenderState(payload);
      audio.play(soundGate.stateChanged(state.state, performance.now()));
      renderer.setState(state);
      stage.setAttribute("aria-label", ariaLabelForState(state));
    }),
    safeListen<PetSettings>("pet://settings", (settings) => {
      tutorialStep = normalizeTutorialStep(settings?.tutorialStep);
      applySoundSetting(settings?.soundEnabled === true);
    }),
    safeListen<unknown>("pet://visibility", (payload) => {
      visible = visibilityFrom(payload);
      stage.hidden = !visible;
    }),
  ]);

  const settings = await safeInvoke<PetSettings>("get_settings");
  tutorialStep = normalizeTutorialStep(settings?.tutorialStep);
  applySoundSetting(settings?.soundEnabled === true);

  window.addEventListener("beforeunload", () => {
    window.cancelAnimationFrame(animationHandle);
    if (dragFrame) window.cancelAnimationFrame(dragFrame);
    resizeObserver.disconnect();
    for (const unlisten of unlisteners) unlisten();
  }, { once: true });
}

async function startBubbleWindow(app: HTMLElement): Promise<void> {
  app.innerHTML = `
    <div class="bubble-stage">
      <div class="bubble" role="status" aria-live="polite" hidden></div>
    </div>
  `;
  const bubble = app.querySelector<HTMLElement>(".bubble");
  if (!bubble) return;

  let hideTimer = 0;
  let tutorialStep = 3;
  const show = (message: BubbleMessage): void => {
    window.clearTimeout(hideTimer);
    if (!message.visible || !message.text) {
      bubble.hidden = true;
      bubble.textContent = "";
      return;
    }
    bubble.textContent = message.text;
    bubble.dataset.kind = message.kind;
    bubble.hidden = false;
    if (message.kind !== "tutorial" && message.durationMs > 0) {
      hideTimer = window.setTimeout(() => {
        bubble.hidden = true;
      }, message.durationMs);
    }
  };

  const showTutorial = (step: number): void => {
    tutorialStep = normalizeTutorialStep(step);
    const text = tutorialText(tutorialStep);
    show(text
      ? { text, visible: true, kind: "tutorial", durationMs: 0 }
      : { text: "", visible: false, kind: "tutorial", durationMs: 0 });
  };

  const unlisteners = await Promise.all([
    safeListen<unknown>("pet://bubble", (payload) => {
      const message = normalizeBubbleMessage(payload);
      if (message) show(message);
    }),
    safeListen<PetSettings>("pet://settings", (settings) => {
      showTutorial(settings?.tutorialStep);
    }),
    safeListen<unknown>("pet://visibility", (payload) => {
      if (!visibilityFrom(payload)) show({ text: "", visible: false, kind: "speech", durationMs: 0 });
      else if (tutorialStep < 3) showTutorial(tutorialStep);
    }),
  ]);

  const settings = await safeInvoke<PetSettings>("get_settings");
  showTutorial(settings?.tutorialStep ?? 0);

  window.addEventListener("beforeunload", () => {
    window.clearTimeout(hideTimer);
    for (const unlisten of unlisteners) unlisten();
  }, { once: true });
}

function pointerPoint(event: PointerEvent): PointerPoint {
  return {
    pointerId: event.pointerId,
    screenX: event.screenX,
    screenY: event.screenY,
  };
}

function visibilityFrom(payload: unknown): boolean {
  if (typeof payload === "boolean") return payload;
  if (payload && typeof payload === "object") {
    return (payload as Record<string, unknown>).visible !== false;
  }
  return true;
}

function ariaLabelForState(state: RenderState): string {
  const labels: Record<RenderState["state"], string> = {
    idle: "发呆",
    walking: "散步",
    running: "奔跑",
    sitting: "坐下",
    sleeping: "睡觉",
    stretching: "伸懒腰",
    tumbling: "摔跟头",
    dragged: "被拖动",
  };
  return `云尾兽，正在${labels[state.state]}`;
}

function announce(message: string): void {
  const status = document.querySelector<HTMLElement>("#app-status");
  if (status) status.textContent = message;
}

function installGlobalErrorFallback(): void {
  window.addEventListener("error", (event) => announce(`云尾兽遇到了问题：${event.message}`));
  window.addEventListener("unhandledrejection", () => announce("云尾兽暂时无法完成这个动作。"));
  window.addEventListener("yunwei:bridge-error", () => announce("部分桌面功能暂时不可用。"));
}
