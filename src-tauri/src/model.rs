use serde::{Deserialize, Serialize};
use crate::environment::{DisplayMode, VisibilityReason};

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;
pub const M1_PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BehaviorState {
    Idle,
    Walking,
    Jumping,
    Landing,
    Dragged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresentationPhase {
    IdleLoop,
    WalkCycle,
    JumpPrepare,
    JumpAscend,
    JumpApex,
    JumpDescend,
    LandCompress,
    LandRecover,
    DragVisual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldPoint {
    pub monitor_id: String,
    pub x_logical: f64,
    pub y_logical: f64,
}

impl WorldPoint {
    pub fn new(monitor_id: impl Into<String>, x_logical: f64, y_logical: f64) -> Self {
        Self { monitor_id: monitor_id.into(), x_logical, y_logical }
    }

    pub fn is_valid(&self) -> bool {
        !self.monitor_id.trim().is_empty() && self.x_logical.is_finite() && self.y_logical.is_finite()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FootingSource {
    DesktopWorkArea,
    ForegroundWindowTop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Footing {
    pub id: String,
    pub monitor_id: String,
    pub top_y_logical: f64,
    pub min_x_logical: f64,
    pub max_x_logical: f64,
    pub source: FootingSource,
}

impl Footing {
    pub fn clamp(&self, point: &WorldPoint) -> WorldPoint {
        WorldPoint::new(
            self.monitor_id.clone(),
            point.x_logical.clamp(self.min_x_logical, self.max_x_logical),
            self.top_y_logical,
        )
    }

    pub fn contains(&self, point: &WorldPoint) -> bool {
        point.monitor_id == self.monitor_id
            && (self.min_x_logical..=self.max_x_logical).contains(&point.x_logical)
            && (point.y_logical - self.top_y_logical).abs() < f64::EPSILON
    }

    pub fn is_valid(&self) -> bool {
        !self.id.trim().is_empty()
            && !self.monitor_id.trim().is_empty()
            && self.top_y_logical.is_finite()
            && self.min_x_logical.is_finite()
            && self.max_x_logical.is_finite()
            && self.min_x_logical <= self.max_x_logical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MotionKind {
    Idle,
    Walk,
    Jump,
    Landing,
    Drag,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionArc {
    pub apex: WorldPoint,
    pub start_offset_ms: u64,
    pub end_offset_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseSlice {
    pub phase: PresentationPhase,
    pub start_offset_ms: u64,
    pub duration_ms: u64,
}

impl PhaseSlice {
    pub fn new(phase: PresentationPhase, start_offset_ms: u64, duration_ms: u64) -> Self {
        Self { phase, start_offset_ms, duration_ms }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPlan {
    pub protocol_version: u8,
    pub sequence: u64,
    pub id: u64,
    pub kind: MotionKind,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub from: WorldPoint,
    pub to: WorldPoint,
    pub arc: Option<MotionArc>,
    pub facing: Facing,
    pub phase_schedule: Vec<PhaseSlice>,
}

impl MotionPlan {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.protocol_version != M1_PROTOCOL_VERSION
            || self.sequence == 0
            || self.id == 0
            || self.duration_ms == 0
        {
            return Err("version, id, or duration is invalid");
        }
        if !self.from.is_valid() || !self.to.is_valid() || self.from.monitor_id != self.to.monitor_id {
            return Err("points are invalid");
        }
        let mut expected_start = 0;
        for slice in &self.phase_schedule {
            if slice.duration_ms == 0 || slice.start_offset_ms != expected_start {
                return Err("phase schedule is not contiguous");
            }
            expected_start = expected_start.saturating_add(slice.duration_ms);
        }
        if self.phase_schedule.is_empty() || expected_start != self.duration_ms {
            return Err("phase schedule does not cover duration");
        }
        if let Some(arc) = &self.arc {
            if !arc.apex.is_valid()
                || arc.apex.monitor_id != self.from.monitor_id
                || arc.start_offset_ms >= arc.end_offset_ms
                || arc.end_offset_ms > self.duration_ms
            {
                return Err("arc is invalid");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub protocol_version: u8,
    pub sequence: u64,
    pub behavior: BehaviorState,
    pub position: WorldPoint,
    pub footing: Footing,
    pub active_plan: Option<MotionPlan>,
    #[serde(default)]
    pub display_mode: DisplayMode,
    #[serde(default)]
    pub manually_hidden: bool,
    #[serde(default)]
    pub visibility_reason: Option<VisibilityReason>,
}

impl RuntimeSnapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.protocol_version != M1_PROTOCOL_VERSION || self.sequence == 0 {
            return Err("snapshot version or sequence is invalid");
        }
        if !self.position.is_valid() || !self.footing.is_valid() || self.position.monitor_id != self.footing.monitor_id {
            return Err("snapshot position or footing is invalid");
        }
        if let Some(plan) = &self.active_plan {
            plan.validate()?;
            if plan.sequence >= self.sequence
                || plan.from.monitor_id != self.position.monitor_id
                || plan.to.monitor_id != self.position.monitor_id
            {
                return Err("snapshot plan is invalid");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum InputObservation {
    DragStarted { pointer_x_physical: f64, pointer_y_physical: f64 },
    DragMoved { pointer_x_physical: f64, pointer_y_physical: f64 },
    DragEnded { pointer_x_physical: f64, pointer_y_physical: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationObservation {
    pub protocol_version: u8,
    pub plan_id: u64,
    pub phase: PresentationPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PetScale {
    Small,
    #[default]
    Medium,
    Large,
}

impl PetScale {
    pub fn logical_pixels(self) -> u32 {
        match self {
            Self::Small => 120,
            Self::Medium => 180,
            Self::Large => 260,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PetSettings {
    pub schema_version: u32,
    pub scale: PetScale,
    pub sound_enabled: bool,
    pub autostart_enabled: bool,
    pub monitor_id: Option<String>,
    pub normalized_x: f64,
    pub tutorial_step: u8,
}

impl Default for PetSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            scale: PetScale::Medium,
            sound_enabled: false,
            autostart_enabled: false,
            monitor_id: None,
            normalized_x: 0.86,
            tutorial_step: 0,
        }
    }
}

impl PetSettings {
    pub fn normalize(&mut self) {
        self.schema_version = SETTINGS_SCHEMA_VERSION;
        if !self.normalized_x.is_finite() {
            self.normalized_x = PetSettings::default().normalized_x;
        }
        self.normalized_x = self.normalized_x.clamp(0.0, 1.0);
        self.tutorial_step = self.tutorial_step.min(3);
        if self
            .monitor_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            self.monitor_id = None;
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub scale: Option<PetScale>,
    pub sound_enabled: Option<bool>,
    pub autostart_enabled: Option<bool>,
    pub monitor_id: Option<Option<String>>,
    pub normalized_x: Option<f64>,
    pub tutorial_step: Option<u8>,
}

impl SettingsPatch {
    pub fn apply(self, settings: &mut PetSettings) {
        if let Some(value) = self.scale {
            settings.scale = value;
        }
        if let Some(value) = self.sound_enabled {
            settings.sound_enabled = value;
        }
        if let Some(value) = self.autostart_enabled {
            settings.autostart_enabled = value;
        }
        if let Some(value) = self.monitor_id {
            settings.monitor_id = value;
        }
        if let Some(value) = self.normalized_x {
            settings.normalized_x = value;
        }
        if let Some(value) = self.tutorial_step {
            settings.tutorial_step = value;
        }
        settings.normalize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    pub fn reverse(&mut self) {
        *self = match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BubblePayload {
    pub visible: bool,
    pub text: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityPayload {
    pub visible: bool,
    pub manually_hidden: bool,
    pub fullscreen_hidden: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m1_motion_plan_requires_a_contiguous_phase_schedule() {
        let plan = MotionPlan {
            protocol_version: 1,
            sequence: 7,
            id: 3,
            kind: MotionKind::Jump,
            started_at_ms: 1_000,
            duration_ms: 1_500,
            from: WorldPoint::new("primary", 100.0, 420.0),
            to: WorldPoint::new("primary", 196.0, 420.0),
            arc: Some(MotionArc {
                apex: WorldPoint::new("primary", 148.0, 348.0),
                start_offset_ms: 220,
                end_offset_ms: 940,
            }),
            facing: Facing::Right,
            phase_schedule: vec![
                PhaseSlice::new(PresentationPhase::JumpPrepare, 0, 220),
                PhaseSlice::new(PresentationPhase::JumpAscend, 220, 180),
                PhaseSlice::new(PresentationPhase::JumpApex, 400, 220),
                PhaseSlice::new(PresentationPhase::JumpDescend, 620, 320),
                PhaseSlice::new(PresentationPhase::LandCompress, 940, 240),
                PhaseSlice::new(PresentationPhase::LandRecover, 1_180, 320),
            ],
        };

        assert!(plan.validate().is_ok());
    }

    #[test]
    fn m1_shared_jump_fixture_is_valid_for_rust_producer_and_consumer() {
        let plan: MotionPlan = serde_json::from_str(include_str!("../../tests/fixtures/protocol/m1-jump.json"))
            .expect("shared jump fixture parses");
        assert!(plan.validate().is_ok());
        assert_eq!(serde_json::to_value(plan).unwrap()["protocolVersion"], 1);
    }

    #[test]
    fn m1_shared_startup_snapshot_fixture_is_valid() {
        let snapshot: RuntimeSnapshot = serde_json::from_str(include_str!("../../tests/fixtures/protocol/m1-startup-snapshot.json"))
            .expect("shared startup snapshot parses");
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn m2_runtime_snapshot_defaults_legacy_environment_fields() {
        let snapshot: RuntimeSnapshot = serde_json::from_str(include_str!("../../tests/fixtures/protocol/m1-startup-snapshot.json"))
            .expect("legacy snapshot parses");
        assert_eq!(snapshot.display_mode, crate::environment::DisplayMode::AboveNormalWindows);
        assert!(!snapshot.manually_hidden);
        assert_eq!(snapshot.visibility_reason, None);
    }

    #[test]
    fn m2_shared_dual_monitor_fixture_deserializes() {
        let fixture = include_str!("../../tests/fixtures/protocol/m2-dual-monitor-150.json");
        let snapshot: RuntimeSnapshot = serde_json::from_str(fixture).expect("M2 fixture");
        assert_eq!(snapshot.footing.source, FootingSource::ForegroundWindowTop);
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn m1_rejects_unknown_versions_and_invalid_coordinates() {
        let mut plan: MotionPlan = serde_json::from_str(include_str!("../../tests/fixtures/protocol/m1-jump.json")).unwrap();
        plan.protocol_version = 2;
        assert!(plan.validate().is_err());
        plan.protocol_version = M1_PROTOCOL_VERSION;
        plan.to.x_logical = f64::NAN;
        assert!(plan.validate().is_err());
    }

    #[test]
    fn settings_are_normalized_after_a_patch() {
        let mut settings = PetSettings::default();
        SettingsPatch {
            normalized_x: Some(9.0),
            tutorial_step: Some(99),
            monitor_id: Some(Some("  ".into())),
            ..Default::default()
        }
        .apply(&mut settings);

        assert_eq!(settings.normalized_x, 1.0);
        assert_eq!(settings.tutorial_step, 3);
        assert_eq!(settings.monitor_id, None);
    }

}
