export interface PointerPoint {
  pointerId: number;
  screenX: number;
  screenY: number;
}

export type GestureAction =
  | { kind: "click"; point: PointerPoint }
  | { kind: "dragStart"; point: PointerPoint }
  | { kind: "dragMove"; point: PointerPoint }
  | { kind: "dragEnd"; point: PointerPoint };

export class GestureTracker {
  private start: PointerPoint | null = null;
  private dragging = false;

  constructor(private readonly threshold = 6) {}

  pointerDown(point: PointerPoint): void {
    this.start = point;
    this.dragging = false;
  }

  pointerMove(point: PointerPoint): GestureAction[] {
    if (!this.start || point.pointerId !== this.start.pointerId) return [];

    if (!this.dragging) {
      const distance = Math.hypot(
        point.screenX - this.start.screenX,
        point.screenY - this.start.screenY,
      );
      if (distance < this.threshold) return [];
      this.dragging = true;
      return [
        { kind: "dragStart", point: this.start },
        { kind: "dragMove", point },
      ];
    }

    return [{ kind: "dragMove", point }];
  }

  pointerUp(point: PointerPoint): GestureAction[] {
    if (!this.start || point.pointerId !== this.start.pointerId) return [];
    const action: GestureAction = this.dragging
      ? { kind: "dragEnd", point }
      : { kind: "click", point };
    this.reset();
    return [action];
  }

  pointerCancel(point: PointerPoint): GestureAction[] {
    if (!this.start || point.pointerId !== this.start.pointerId) return [];
    const actions: GestureAction[] = this.dragging ? [{ kind: "dragEnd", point }] : [];
    this.reset();
    return actions;
  }

  isDragging(): boolean {
    return this.dragging;
  }

  private reset(): void {
    this.start = null;
    this.dragging = false;
  }
}
