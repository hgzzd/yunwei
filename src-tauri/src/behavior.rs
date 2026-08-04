use crate::model::{
    BehaviorState, Facing, Footing, MotionArc, MotionKind, MotionPlan, PhaseSlice,
    PresentationPhase, RuntimeSnapshot, WorldPoint,
};

pub trait RandomSource {
    fn range(&mut self, min: u64, max: u64) -> u64;
}

#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub first_action_min_ms: u64,
    pub first_action_max_ms: u64,
    pub action_min_ms: u64,
    pub action_max_ms: u64,
    pub walk_probability_percent: u64,
    pub walk_min_ms: u64,
    pub walk_max_ms: u64,
    pub walk_speed_logical_per_second: f64,
    pub jump_cooldown_ms: u64,
    pub jump_distance_logical: f64,
    pub jump_height_logical: f64,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            first_action_min_ms: 8_000,
            first_action_max_ms: 15_000,
            action_min_ms: ACTION_MIN_MS,
            action_max_ms: ACTION_MAX_MS,
            walk_probability_percent: 75,
            walk_min_ms: 2_400,
            walk_max_ms: 3_600,
            walk_speed_logical_per_second: 65.0,
            jump_cooldown_ms: 60_000,
            jump_distance_logical: 96.0,
            jump_height_logical: 72.0,
        }
    }
}

pub struct BehaviorPlanner<R> {
    config: PlannerConfig,
    rng: R,
    footing: Footing,
    position: WorldPoint,
    behavior: BehaviorState,
    facing: Facing,
    active_plan: MotionPlan,
    last_jump_completed_at_ms: Option<u64>,
    previous_action_was_jump: bool,
    next_plan_id: u64,
    next_sequence: u64,
    next_action_due_at_ms: u64,
    dragging: bool,
}

impl<R: RandomSource> BehaviorPlanner<R> {
    pub fn new(
        config: PlannerConfig,
        mut rng: R,
        position: WorldPoint,
        footing: Footing,
        now_ms: u64,
    ) -> Self {
        let first_delay = rng.range(config.first_action_min_ms, config.first_action_max_ms);
        let mut planner = Self {
            config,
            rng,
            footing,
            position: position.clone(),
            behavior: BehaviorState::Idle,
            facing: Facing::Right,
            active_plan: MotionPlan {
                protocol_version: crate::model::PROTOCOL_VERSION,
                sequence: 0,
                id: 0,
                kind: MotionKind::Idle,
                started_at_ms: now_ms,
                duration_ms: 1,
                from: position.clone(),
                to: position,
                arc: None,
                facing: Facing::Right,
                phase_schedule: vec![PhaseSlice::new(PresentationPhase::IdleLoop, 0, 1)],
            },
            last_jump_completed_at_ms: None,
            previous_action_was_jump: false,
            next_plan_id: 1,
            next_sequence: 1,
            next_action_due_at_ms: now_ms.saturating_add(first_delay),
            dragging: false,
        };
        planner.active_plan = planner.idle_plan(now_ms, first_delay);
        planner
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<MotionPlan> {
        if self.dragging {
            return None;
        }
        if self.behavior == BehaviorState::Jumping
            && now_ms.saturating_sub(self.active_plan.started_at_ms) >= 940
        {
            self.behavior = BehaviorState::Landing;
        }
        if now_ms < self.next_action_due_at_ms {
            return None;
        }

        self.position = self.active_plan.to.clone();
        if matches!(self.active_plan.kind, MotionKind::Jump) {
            self.last_jump_completed_at_ms = Some(self.next_action_due_at_ms);
            self.previous_action_was_jump = true;
        } else if matches!(self.active_plan.kind, MotionKind::Walk) {
            self.previous_action_was_jump = false;
        }

        let plan = if matches!(self.active_plan.kind, MotionKind::Idle) {
            self.choose_action(now_ms)
        } else {
            self.behavior = BehaviorState::Idle;
            let mut delay = self
                .rng
                .range(self.config.action_min_ms, self.config.action_max_ms);
            if let Some(last_jump) = self.last_jump_completed_at_ms {
                delay = delay.max(
                    last_jump
                        .saturating_add(self.config.jump_cooldown_ms)
                        .saturating_sub(now_ms),
                );
            }
            self.previous_action_was_jump = matches!(self.active_plan.kind, MotionKind::Jump);
            self.idle_plan(now_ms, delay)
        };
        self.next_action_due_at_ms = now_ms.saturating_add(plan.duration_ms);
        self.active_plan = plan.clone();
        Some(plan)
    }

    pub fn active_plan(&self) -> &MotionPlan {
        &self.active_plan
    }
    pub fn behavior(&self) -> BehaviorState {
        self.behavior
    }
    pub fn position(&self) -> &WorldPoint {
        &self.position
    }
    pub fn footing(&self) -> &Footing {
        &self.footing
    }
    pub fn next_action_due_at_ms(&self) -> u64 {
        self.next_action_due_at_ms
    }

    pub fn begin_drag(&mut self, now_ms: u64) -> MotionPlan {
        self.dragging = true;
        self.behavior = BehaviorState::Dragged;
        self.position = self.position_at(now_ms);
        self.drag_plan(now_ms)
    }

    pub fn drag_to(&mut self, now_ms: u64, point: WorldPoint) -> MotionPlan {
        if point.is_valid() && point.monitor_id == self.footing.monitor_id {
            self.position = point;
        }
        self.drag_plan(now_ms)
    }

    pub fn land_after_drag(&mut self, now_ms: u64) -> MotionPlan {
        self.dragging = false;
        self.behavior = BehaviorState::Landing;
        let from = self.position.clone();
        let to = self.footing.clamp(&from);
        let plan = self.plan(
            MotionKind::Landing,
            now_ms,
            560,
            from,
            to,
            None,
            vec![
                PhaseSlice::new(PresentationPhase::LandCompress, 0, 240),
                PhaseSlice::new(PresentationPhase::LandRecover, 240, 320),
            ],
        );
        self.next_action_due_at_ms = now_ms.saturating_add(plan.duration_ms);
        self.active_plan = plan.clone();
        plan
    }

    pub fn runtime_snapshot(&mut self, now_ms: u64) -> RuntimeSnapshot {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        RuntimeSnapshot {
            protocol_version: crate::model::PROTOCOL_VERSION,
            sequence,
            behavior: self.behavior,
            position: self.position_at(now_ms),
            footing: self.footing.clone(),
            active_plan: (self.active_plan.id > 0).then(|| self.active_plan.clone()),
            display_mode: crate::environment::DisplayMode::AboveNormalWindows,
            manually_hidden: false,
            visibility_reason: None,
        }
    }

    /// Re-anchor the authoritative world when Windows changes the available
    /// footing. The browser receives the resulting landing plan; it never
    /// derives a replacement position itself.
    pub fn reanchor(&mut self, now_ms: u64, footing: Footing, position: WorldPoint) -> MotionPlan {
        self.dragging = false;
        self.behavior = BehaviorState::Landing;
        self.footing = footing;
        let from = position;
        let to = self.footing.clamp(&from);
        self.position = from.clone();
        let plan = self.plan(
            MotionKind::Landing,
            now_ms,
            560,
            from,
            to,
            None,
            vec![
                PhaseSlice::new(PresentationPhase::LandCompress, 0, 240),
                PhaseSlice::new(PresentationPhase::LandRecover, 240, 320),
            ],
        );
        self.next_action_due_at_ms = now_ms.saturating_add(plan.duration_ms);
        self.active_plan = plan.clone();
        plan
    }

    pub fn position_at(&self, now_ms: u64) -> WorldPoint {
        if self.dragging {
            return self.position.clone();
        }
        let plan = &self.active_plan;
        let elapsed = now_ms
            .saturating_sub(plan.started_at_ms)
            .min(plan.duration_ms);
        if let Some(arc) = &plan.arc {
            if elapsed <= arc.start_offset_ms {
                return plan.from.clone();
            }
            if elapsed >= arc.end_offset_ms {
                return plan.to.clone();
            }
            let t = (elapsed - arc.start_offset_ms) as f64
                / (arc.end_offset_ms - arc.start_offset_ms) as f64;
            let a = 1.0 - t;
            return WorldPoint::new(
                plan.from.monitor_id.clone(),
                a * a * plan.from.x_logical
                    + 2.0 * a * t * arc.apex.x_logical
                    + t * t * plan.to.x_logical,
                a * a * plan.from.y_logical
                    + 2.0 * a * t * arc.apex.y_logical
                    + t * t * plan.to.y_logical,
            );
        }
        let t = elapsed as f64 / plan.duration_ms as f64;
        WorldPoint::new(
            plan.from.monitor_id.clone(),
            plan.from.x_logical + (plan.to.x_logical - plan.from.x_logical) * t,
            plan.from.y_logical + (plan.to.y_logical - plan.from.y_logical) * t,
        )
    }

    fn choose_action(&mut self, now_ms: u64) -> MotionPlan {
        let roll = self.rng.range(0, 99);
        let jump_allowed = !self.previous_action_was_jump
            && self
                .last_jump_completed_at_ms
                .is_none_or(|last| now_ms.saturating_sub(last) >= self.config.jump_cooldown_ms);
        if roll >= self.config.walk_probability_percent && jump_allowed {
            self.behavior = BehaviorState::Jumping;
            self.jump_plan(now_ms)
        } else {
            self.behavior = BehaviorState::Walking;
            self.walk_plan(now_ms)
        }
    }

    fn idle_plan(&mut self, now_ms: u64, duration_ms: u64) -> MotionPlan {
        self.plan(
            MotionKind::Idle,
            now_ms,
            duration_ms,
            self.position.clone(),
            self.position.clone(),
            None,
            vec![PhaseSlice::new(PresentationPhase::IdleLoop, 0, duration_ms)],
        )
    }

    fn walk_plan(&mut self, now_ms: u64) -> MotionPlan {
        let duration_ms = self
            .rng
            .range(self.config.walk_min_ms, self.config.walk_max_ms);
        let distance = self.config.walk_speed_logical_per_second * duration_ms as f64 / 1_000.0;
        let direction = if self.rng.range(0, 1) == 0 { 1.0 } else { -1.0 };
        let requested = WorldPoint::new(
            self.footing.monitor_id.clone(),
            self.position.x_logical + direction * distance,
            self.footing.top_y_logical,
        );
        let to = self.footing.clamp(&requested);
        self.facing = if to.x_logical >= self.position.x_logical {
            Facing::Right
        } else {
            Facing::Left
        };
        self.plan(
            MotionKind::Walk,
            now_ms,
            duration_ms,
            self.position.clone(),
            to,
            None,
            vec![PhaseSlice::new(
                PresentationPhase::WalkCycle,
                0,
                duration_ms,
            )],
        )
    }

    fn jump_plan(&mut self, now_ms: u64) -> MotionPlan {
        let direction = if self.facing == Facing::Right {
            1.0
        } else {
            -1.0
        };
        let requested = WorldPoint::new(
            self.footing.monitor_id.clone(),
            self.position.x_logical + direction * self.config.jump_distance_logical,
            self.footing.top_y_logical,
        );
        let mut to = self.footing.clamp(&requested);
        if (to.x_logical - self.position.x_logical).abs() < f64::EPSILON {
            to = self.footing.clamp(&WorldPoint::new(
                self.footing.monitor_id.clone(),
                self.position.x_logical - direction * self.config.jump_distance_logical,
                self.footing.top_y_logical,
            ));
            self.facing.reverse();
        }
        let apex = WorldPoint::new(
            self.footing.monitor_id.clone(),
            (self.position.x_logical + to.x_logical) / 2.0,
            self.footing.top_y_logical - self.config.jump_height_logical,
        );
        self.plan(
            MotionKind::Jump,
            now_ms,
            1_500,
            self.position.clone(),
            to,
            Some(MotionArc {
                apex,
                start_offset_ms: 220,
                end_offset_ms: 940,
            }),
            vec![
                PhaseSlice::new(PresentationPhase::JumpPrepare, 0, 220),
                PhaseSlice::new(PresentationPhase::JumpAscend, 220, 180),
                PhaseSlice::new(PresentationPhase::JumpApex, 400, 220),
                PhaseSlice::new(PresentationPhase::JumpDescend, 620, 320),
                PhaseSlice::new(PresentationPhase::LandCompress, 940, 240),
                PhaseSlice::new(PresentationPhase::LandRecover, 1_180, 320),
            ],
        )
    }

    fn drag_plan(&mut self, now_ms: u64) -> MotionPlan {
        let point = self.position.clone();
        let plan = self.plan(
            MotionKind::Drag,
            now_ms,
            DRAG_PLAN_DURATION_MS,
            point.clone(),
            point,
            None,
            vec![PhaseSlice::new(
                PresentationPhase::DragVisual,
                0,
                DRAG_PLAN_DURATION_MS,
            )],
        );
        self.next_action_due_at_ms = u64::MAX;
        self.active_plan = plan.clone();
        plan
    }

    fn plan(
        &mut self,
        kind: MotionKind,
        started_at_ms: u64,
        duration_ms: u64,
        from: WorldPoint,
        to: WorldPoint,
        arc: Option<MotionArc>,
        phase_schedule: Vec<PhaseSlice>,
    ) -> MotionPlan {
        let plan = MotionPlan {
            protocol_version: crate::model::PROTOCOL_VERSION,
            sequence: self.next_sequence,
            id: self.next_plan_id,
            kind,
            started_at_ms,
            duration_ms,
            from,
            to,
            arc,
            facing: self.facing,
            phase_schedule,
        };
        self.next_plan_id = self.next_plan_id.saturating_add(1);
        self.next_sequence = self.next_sequence.saturating_add(1);
        debug_assert!(plan.validate().is_ok());
        plan
    }
}

const ACTION_MIN_MS: u64 = 15_000;
const ACTION_MAX_MS: u64 = 45_000;
const DRAG_PLAN_DURATION_MS: u64 = 60_000;

#[derive(Debug)]
pub(crate) struct Lcg(u64);

impl Lcg {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn range(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(min <= max);
        min + self.next() % (max - min + 1)
    }
}

impl RandomSource for Lcg {
    fn range(&mut self, min: u64, max: u64) -> u64 {
        self.range(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Footing, FootingSource, MotionKind, WorldPoint};

    struct ScriptedRng {
        values: std::collections::VecDeque<u64>,
    }

    impl ScriptedRng {
        fn new(values: Vec<u64>) -> Self {
            Self {
                values: values.into(),
            }
        }
    }

    impl RandomSource for ScriptedRng {
        fn range(&mut self, min: u64, max: u64) -> u64 {
            self.values.pop_front().unwrap_or(min).clamp(min, max)
        }
    }

    #[test]
    fn m1_planner_prevents_consecutive_jumps_and_applies_jump_cooldown() {
        let footing = Footing {
            id: "desktop".into(),
            monitor_id: "primary".into(),
            top_y_logical: 420.0,
            min_x_logical: 0.0,
            max_x_logical: 500.0,
            source: FootingSource::DesktopWorkArea,
        };
        let mut planner = BehaviorPlanner::new(
            PlannerConfig::default(),
            ScriptedRng::new(vec![8_000, 99, 2_400, 0]),
            WorldPoint::new("primary", 100.0, 420.0),
            footing,
            0,
        );

        let jump = planner.tick(8_000).expect("first plan");
        assert_eq!(jump.kind, MotionKind::Jump);
        let after_jump = planner.tick(9_500).expect("idle after jump");
        assert_eq!(after_jump.kind, MotionKind::Idle);
        assert!(planner.next_action_due_at_ms() >= 69_500);
    }

    #[test]
    fn drag_start_replaces_the_active_plan_with_a_new_drag_visual_plan() {
        let footing = Footing {
            id: "desktop".into(),
            monitor_id: "primary".into(),
            top_y_logical: 420.0,
            min_x_logical: 0.0,
            max_x_logical: 500.0,
            source: FootingSource::DesktopWorkArea,
        };
        let mut planner = BehaviorPlanner::new(
            PlannerConfig::default(),
            ScriptedRng::new(vec![8_000]),
            WorldPoint::new("primary", 100.0, 420.0),
            footing,
            0,
        );
        let before = planner.active_plan().clone();

        let drag = planner.begin_drag(100);

        assert!(drag.id > before.id);
        assert!(drag.sequence > before.sequence);
        assert_eq!(drag.kind, MotionKind::Drag);
        assert_eq!(drag.phase_schedule.len(), 1);
        assert_eq!(drag.phase_schedule[0].phase, PresentationPhase::DragVisual);
        assert_eq!(drag.from, planner.position_at(100));
        assert_eq!(drag.to, drag.from);
        assert_eq!(planner.behavior(), BehaviorState::Dragged);
    }

    #[test]
    fn startup_snapshot_contains_only_a_valid_versioned_plan() {
        let footing = Footing {
            id: "desktop".into(),
            monitor_id: "primary".into(),
            top_y_logical: 420.0,
            min_x_logical: 0.0,
            max_x_logical: 500.0,
            source: FootingSource::DesktopWorkArea,
        };
        let mut planner = BehaviorPlanner::new(
            PlannerConfig::default(),
            ScriptedRng::new(vec![8_000]),
            WorldPoint::new("primary", 100.0, 420.0),
            footing,
            0,
        );
        let snapshot = planner.runtime_snapshot(0);
        assert!(snapshot
            .active_plan
            .as_ref()
            .is_some_and(|plan| plan.sequence > 0 && plan.id > 0));
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn drag_end_uses_the_planner_position_and_snapshots_sample_live_position() {
        let footing = Footing {
            id: "desktop".into(),
            monitor_id: "primary".into(),
            top_y_logical: 420.0,
            min_x_logical: 0.0,
            max_x_logical: 500.0,
            source: FootingSource::DesktopWorkArea,
        };
        let mut planner = BehaviorPlanner::new(
            PlannerConfig::default(),
            ScriptedRng::new(vec![8_000]),
            WorldPoint::new("primary", 100.0, 420.0),
            footing.clone(),
            0,
        );
        planner.begin_drag(100);
        planner.drag_to(110, WorldPoint::new("primary", 640.0, 200.0));
        let landing = planner.land_after_drag(120);
        let snapshot = planner.runtime_snapshot(240);

        assert_eq!(landing.from, WorldPoint::new("primary", 640.0, 200.0));
        assert_eq!(landing.to, footing.clamp(&landing.from));
        assert!(landing.sequence < snapshot.sequence);
        assert_eq!(snapshot.position, planner.position_at(240));
        assert!(snapshot.validate().is_ok());
    }
}
