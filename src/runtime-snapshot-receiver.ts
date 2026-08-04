import { isRuntimeVisible, parseRuntimeSnapshot, type RuntimeSnapshot } from "./pet-model";

export interface AcceptedRuntimeSnapshot {
  snapshot: RuntimeSnapshot;
  visible: boolean;
}

/** Strict, monotonic RuntimeSnapshot consumer shared by both webview windows. */
export class RuntimeSnapshotReceiver {
  private latest: AcceptedRuntimeSnapshot | null = null;

  accept(payload: unknown): AcceptedRuntimeSnapshot | null {
    const snapshot = parseRuntimeSnapshot(payload);
    if (!snapshot || (this.latest && snapshot.sequence <= this.latest.snapshot.sequence)) return null;
    this.latest = { snapshot, visible: isRuntimeVisible(snapshot) };
    return this.latest;
  }

  current(): AcceptedRuntimeSnapshot | null {
    return this.latest;
  }
}
