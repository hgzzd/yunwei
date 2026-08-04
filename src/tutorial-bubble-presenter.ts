import { parseTutorialBubbleDirective, type TutorialBubbleDirective } from "./pet-model";
import { RuntimeSnapshotReceiver } from "./runtime-snapshot-receiver";

export interface TutorialBubbleView {
  visible: boolean;
  text: string;
  id: number | null;
}

/** Strictly renders Rust-issued tutorial directives; it never chooses text or timing. */
export class TutorialBubblePresenter {
  private directive: TutorialBubbleDirective | null = null;
  private readonly snapshots = new RuntimeSnapshotReceiver();

  acceptDirective(payload: unknown): TutorialBubbleView | null {
    const directive = parseTutorialBubbleDirective(payload);
    if (!directive || (this.directive && directive.sequence <= this.directive.sequence)) return null;
    this.directive = directive;
    return this.current();
  }

  acceptRuntimeSnapshot(payload: unknown): TutorialBubbleView | null {
    if (!this.snapshots.accept(payload)) return null;
    return this.current();
  }

  current(): TutorialBubbleView {
    const visible = this.directive?.visible === true
      && this.snapshots.current()?.visible !== false;
    return {
      visible,
      text: visible ? this.directive?.text ?? "" : "",
      id: visible ? this.directive?.id ?? null : null,
    };
  }
}
