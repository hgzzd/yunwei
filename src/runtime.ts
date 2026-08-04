import placeholderUrl from "./assets/yunwei-placeholder.svg";
import { GestureTracker, type GestureAction, type PointerPoint } from "./gesture";
import {
  PROTOCOL_VERSION,
  type PetSettings,
  type PresentationPhase,
  type RenderState,
  type SpriteManifest,
} from "./pet-model";
import { SpriteRenderer } from "./sprite-renderer";
import { PlanPlayer } from "./plan-player";
import { fetchSpriteManifest } from "./sprite-manifest";
import { currentWindowKind, getAuthoritativeRuntimeSnapshot, observeAnimation, observeInput, safeInvoke, safeListen } from "./tauri-bridge";
import { PetAudioPlayer, SoundGate } from "./pet-audio";
import { TutorialBubblePresenter } from "./tutorial-bubble-presenter";

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
  const planPlayer = new PlanPlayer();
  let renderedPlanPhase = "";
  let observedPlan: { id: number; phase: PresentationPhase } | null = null;
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
    const sample = planPlayer.sample(Date.now());
    if (sample && sample.phase !== renderedPlanPhase) {
      if (observedPlan && (observedPlan.id !== sample.planId || observedPlan.phase !== sample.phase)) {
        void observeAnimation({
          protocolVersion: PROTOCOL_VERSION, planId: observedPlan.id, phase: observedPlan.phase,
        });
      }
      observedPlan = { id: sample.planId, phase: sample.phase };
      renderedPlanPhase = sample.phase;
      renderer.setState({ state: spriteStateForPhase(sample.phase), facing: sample.facing });
      stage.setAttribute("aria-label", `云尾兽，正在${sample.phase}`);
    }
    if (visible) renderer.draw(now);
    animationHandle = window.requestAnimationFrame(animate);
  };
  animationHandle = window.requestAnimationFrame(animate);

  const gesture = new GestureTracker(6);
  const soundGate = new SoundGate();
  const audio = new PetAudioPlayer();
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
    void observeInput({
      kind: "dragMoved", pointerXPhysical: point.screenX, pointerYPhysical: point.screenY,
    });
  };

  const dispatch = (actions: readonly GestureAction[]): void => {
    for (const action of actions) {
      const { point } = action;
      if (action.kind === "click") {
        audio.play(soundGate.clicked(performance.now()));
        void observeInput({ kind: "singleClick" });
      } else if (action.kind === "dragStart") {
        document.body.classList.add("is-dragging");
        void observeInput({
          kind: "dragStarted", pointerXPhysical: point.screenX, pointerYPhysical: point.screenY,
        });
      } else if (action.kind === "dragMove") {
        pendingDragMove = point;
        if (!dragFrame) dragFrame = window.requestAnimationFrame(flushDragMove);
      } else {
        document.body.classList.remove("is-dragging");
        pendingDragMove = null;
        if (dragFrame) window.cancelAnimationFrame(dragFrame);
        dragFrame = 0;
        void observeInput({
          kind: "dragEnded", pointerXPhysical: point.screenX, pointerYPhysical: point.screenY,
        });
      }
    }
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
  });
  stage.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      audio.play(soundGate.clicked(performance.now()));
      void observeInput({ kind: "singleClick" });
    } else if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      void safeInvoke("show_context_menu");
    }
  });

  const unlisteners = await Promise.all([
    safeListen<unknown>("pet://motion-plan", (payload) => {
      if (!planPlayer.acceptMotionPlan(payload)) console.warn("[yunwei] 忽略非法或过期的运动计划。");
    }),
    safeListen<unknown>("pet://runtime-snapshot", (payload) => {
      if (!planPlayer.acceptRuntimeSnapshot(payload)) {
        console.warn("[yunwei] 忽略非法或过期的运行时快照。");
        return;
      }
      visible = planPlayer.isVisible();
      stage.hidden = !visible;
    }),
    safeListen<PetSettings>("pet://settings", (settings) => {
      applySoundSetting(settings?.soundEnabled === true);
    }),
  ]);

  const settings = await safeInvoke<PetSettings>("get_settings");
  const snapshot = await getAuthoritativeRuntimeSnapshot();
  if (snapshot && !planPlayer.acceptRuntimeSnapshot(snapshot)) {
    console.warn("[yunwei] 忽略非法的启动运动快照。");
  }
  visible = planPlayer.isVisible();
  stage.hidden = !visible;
  applySoundSetting(settings?.soundEnabled === true);

  window.addEventListener("beforeunload", () => {
    window.cancelAnimationFrame(animationHandle);
    if (dragFrame) window.cancelAnimationFrame(dragFrame);
    resizeObserver.disconnect();
    for (const unlisten of unlisteners) unlisten();
  }, { once: true });
}

function spriteStateForPhase(phase: import("./pet-model").PresentationPhase): RenderState["state"] {
  const mapping: Record<import("./pet-model").PresentationPhase, RenderState["state"]> = {
    idleLoop: "idle", walkCycle: "walking", jumpPrepare: "stretching", jumpAscend: "tumbling",
    jumpApex: "tumbling", jumpDescend: "tumbling", landCompress: "sitting", landRecover: "idle", dragVisual: "dragged",
  };
  return mapping[phase];
}

async function startBubbleWindow(app: HTMLElement): Promise<void> {
  app.innerHTML = `
    <div class="bubble-stage">
      <div class="bubble" role="status" aria-live="polite" hidden></div>
    </div>
  `;
  const bubble = app.querySelector<HTMLElement>(".bubble");
  if (!bubble) return;

  const presenter = new TutorialBubblePresenter();
  const render = (): void => {
    const view = presenter.current();
    bubble.hidden = !view.visible;
    bubble.textContent = view.text;
    bubble.dataset.directiveId = view.id === null ? "" : String(view.id);
  };

  const unlisteners = await Promise.all([
    safeListen<unknown>("pet://tutorial-bubble-directive", (payload) => {
      if (presenter.acceptDirective(payload)) render();
    }),
    safeListen<unknown>("pet://runtime-snapshot", (payload) => {
      if (presenter.acceptRuntimeSnapshot(payload)) render();
    }),
  ]);

  const directive = await safeInvoke<unknown>("get_tutorial_bubble_directive");
  if (presenter.acceptDirective(directive)) render();
  const snapshot = await getAuthoritativeRuntimeSnapshot();
  if (snapshot && presenter.acceptRuntimeSnapshot(snapshot)) render();

  window.addEventListener("beforeunload", () => {
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

function announce(message: string): void {
  const status = document.querySelector<HTMLElement>("#app-status");
  if (status) status.textContent = message;
}

function installGlobalErrorFallback(): void {
  window.addEventListener("error", (event) => announce(`云尾兽遇到了问题：${event.message}`));
  window.addEventListener("unhandledrejection", () => announce("云尾兽暂时无法完成这个动作。"));
  window.addEventListener("yunwei:bridge-error", () => announce("部分桌面功能暂时不可用。"));
}
