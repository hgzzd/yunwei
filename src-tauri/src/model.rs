use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

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
pub enum PetState {
    Idle,
    Walking,
    Running,
    Sitting,
    Sleeping,
    Stretching,
    Tumbling,
    Dragged,
}

impl PetState {
    pub fn frames(self) -> u16 {
        match self {
            Self::Idle | Self::Sitting => 4,
            Self::Walking | Self::Tumbling => 8,
            Self::Running | Self::Sleeping | Self::Stretching => 6,
            Self::Dragged => 2,
        }
    }

    pub fn moves(self) -> bool {
        matches!(self, Self::Walking | Self::Running)
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
pub struct StatePayload {
    pub state: PetState,
    pub facing: Facing,
    pub frame: u16,
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

    #[test]
    fn frame_counts_match_the_sprite_contract() {
        assert_eq!(PetState::Walking.frames(), 8);
        assert_eq!(PetState::Running.frames(), 6);
        assert_eq!(PetState::Dragged.frames(), 2);
    }
}
