import { parseMotionPlan, parseRuntimeSnapshot, type MotionPlan, type PresentationPhase, type RuntimeSnapshot, type WorldPoint } from "./pet-model";
import { RuntimeSnapshotReceiver } from "./runtime-snapshot-receiver";

export interface PlanSample {
  planId: number;
  phase: PresentationPhase;
  phaseElapsedMs: number;
  facing: MotionPlan["facing"];
  position: WorldPoint;
}

export class PlanPlayer {
  private planSequence = 0;
  private plan: MotionPlan | null = null;
  private readonly snapshots = new RuntimeSnapshotReceiver();

  acceptMotionPlan(payload: unknown): boolean {
    const plan = parseMotionPlan(payload);
    if (!plan || plan.sequence <= this.planSequence || plan.sequence <= (this.snapshots.current()?.snapshot.sequence ?? 0)) return false;
    this.planSequence = plan.sequence;
    this.plan = plan;
    return true;
  }

  acceptRuntimeSnapshot(payload: unknown): boolean {
    const snapshot = parseRuntimeSnapshot(payload);
    if (!snapshot) return false;
    if (snapshot.activePlan && this.plan && snapshot.activePlan.sequence < this.plan.sequence) return false;
    if (!this.snapshots.accept(snapshot)) return false;
    this.planSequence = Math.max(this.planSequence, snapshot.activePlan?.sequence ?? 0);
    this.plan = snapshot.activePlan ?? null;
    return true;
  }

  snapshot(): RuntimeSnapshot | null { return this.snapshots.current()?.snapshot ?? null; }
  isVisible(): boolean { return this.snapshots.current()?.visible ?? true; }

  sample(nowMs: number): PlanSample | null {
    if (!this.plan || !this.isVisible()) return null;
    const elapsed = Math.max(0, Math.min(this.plan.durationMs, nowMs - this.plan.startedAtMs));
    const slice = this.plan.phaseSchedule.find((item) => elapsed < item.startOffsetMs + item.durationMs)
      ?? this.plan.phaseSchedule[this.plan.phaseSchedule.length - 1];
    return {
      planId: this.plan.id,
      phase: slice.phase,
      phaseElapsedMs: Math.max(0, elapsed - slice.startOffsetMs),
      facing: this.plan.facing,
      position: positionAt(this.plan, elapsed),
    };
  }
}

function positionAt(plan: MotionPlan, elapsed: number): WorldPoint {
  const arc = plan.arc;
  if (!arc) return linearPosition(plan, elapsed / plan.durationMs);
  if (elapsed <= arc.startOffsetMs) return plan.from;
  if (elapsed >= arc.endOffsetMs) return plan.to;
  const t = (elapsed - arc.startOffsetMs) / (arc.endOffsetMs - arc.startOffsetMs);
  const a = 1 - t;
  return {
    monitorId: plan.from.monitorId,
    xLogical: a * a * plan.from.xLogical + 2 * a * t * arc.apex.xLogical + t * t * plan.to.xLogical,
    yLogical: a * a * plan.from.yLogical + 2 * a * t * arc.apex.yLogical + t * t * plan.to.yLogical,
  };
}

function linearPosition(plan: MotionPlan, progress: number): WorldPoint {
  const t = Math.max(0, Math.min(1, progress));
  return {
    monitorId: plan.from.monitorId,
    xLogical: plan.from.xLogical + (plan.to.xLogical - plan.from.xLogical) * t,
    yLogical: plan.from.yLogical + (plan.to.yLogical - plan.from.yLogical) * t,
  };
}
