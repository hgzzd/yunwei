import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowKind } from "./pet-model";

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function currentWindowKind(): WindowKind {
  const queryKind = new URLSearchParams(window.location.search).get("window");
  if (queryKind === "bubble") return "bubble";
  if (queryKind === "pet") return "pet";
  if (!isTauriRuntime()) return "pet";

  try {
    return getCurrentWindow().label === "bubble" ? "bubble" : "pet";
  } catch {
    return "pet";
  }
}

export async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | undefined> {
  if (!isTauriRuntime()) return undefined;
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    reportBridgeError(`command:${command}`, error);
    return undefined;
  }
}

export async function safeListen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  try {
    return await listen<T>(event, ({ payload }) => handler(payload));
  } catch (error) {
    reportBridgeError(`event:${event}`, error);
    return () => undefined;
  }
}

function reportBridgeError(source: string, error: unknown): void {
  console.error(`[yunwei] ${source}`, error);
  window.dispatchEvent(new CustomEvent("yunwei:bridge-error", { detail: source }));
}
