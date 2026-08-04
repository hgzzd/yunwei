use crate::model::{Footing, FootingSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhysicalRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PhysicalRect {
    pub(crate) const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub(crate) fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }
    pub(crate) fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    fn overlap_area(self, other: Self) -> i64 {
        let width = (self.right.min(other.right) - self.left.max(other.left)).max(0);
        let height = (self.bottom.min(other.bottom) - self.top.max(other.top)).max(0);
        i64::from(width) * i64::from(height)
    }

    fn contains_center(self, other: Self) -> bool {
        let x = other.left + other.width() / 2;
        let y = other.top + other.height() / 2;
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorSnapshot {
    pub id: String,
    pub work_area_physical: PhysicalRect,
    pub monitor_area_physical: PhysicalRect,
    pub scale_factor: f64,
    pub is_primary: bool,
}

impl MonitorSnapshot {
    pub(crate) fn new(
        id: impl Into<String>,
        work_area_physical: PhysicalRect,
        scale_factor: f64,
        is_primary: bool,
    ) -> Self {
        Self {
            id: id.into(),
            work_area_physical,
            monitor_area_physical: work_area_physical,
            scale_factor,
            is_primary,
        }
    }

    pub(crate) fn logical_width(&self) -> f64 {
        f64::from(self.work_area_physical.width()) / self.scale_factor.max(1.0)
    }
    pub(crate) fn logical_height(&self) -> f64 {
        f64::from(self.work_area_physical.height()) / self.scale_factor.max(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DisplayMode {
    AboveNormalWindows,
    DesktopOnly,
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::AboveNormalWindows
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HideRule {
    pub app_id: String,
}

impl HideRule {
    pub(crate) fn matches(&self, app_id: Option<&str>) -> bool {
        app_id.is_some_and(|value| value == self.app_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum VisibilityReason {
    Fullscreen,
    SpecifiedApp,
    DesktopOnlyForeground,
    MonitorUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForegroundKind {
    Normal,
    DesktopShell,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ForegroundWindowSnapshot {
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub rect_physical: PhysicalRect,
    pub visible: bool,
    pub is_fullscreen: bool,
    pub kind: ForegroundKind,
}

impl ForegroundWindowSnapshot {
    pub(crate) fn normal(
        app_id: Option<String>,
        rect_physical: PhysicalRect,
        is_fullscreen: bool,
    ) -> Self {
        Self {
            app_id,
            title: None,
            rect_physical,
            visible: true,
            is_fullscreen,
            kind: ForegroundKind::Normal,
        }
    }

    pub(crate) fn is_normal(&self) -> bool {
        self.visible && self.kind == ForegroundKind::Normal
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentSnapshot {
    pub monitors: Vec<MonitorSnapshot>,
    pub foreground: Option<ForegroundWindowSnapshot>,
    pub captured_at_ms: u64,
}

/// Boundary for native environment acquisition.  The Windows adapter is the
/// only production implementation; policies depend solely on snapshots.
pub(crate) trait EnvironmentPort {
    fn snapshot(&self, captured_at_ms: u64) -> EnvironmentSnapshot;
}

/// Boundary for applying the already-decided native window state.
pub(crate) trait WindowPort {
    fn set_visible(&self, visible: bool) -> Result<(), String>;
    fn place(&self, x_physical: i32, y_physical: i32) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecoveryPosition {
    pub monitor_id: Option<String>,
    pub normalized_x: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvironmentDecision {
    pub monitor_id: Option<String>,
    pub footing: Option<Footing>,
    pub visibility_reason: Option<VisibilityReason>,
    pub manually_hidden: bool,
}

impl EnvironmentDecision {
    pub(crate) fn is_visible(&self) -> bool {
        !self.manually_hidden && self.visibility_reason.is_none()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentPolicy {
    pub display_mode: DisplayMode,
    pub hide_rules: Vec<HideRule>,
}

impl EnvironmentPolicy {
    pub(crate) fn new(display_mode: DisplayMode, hide_rules: Vec<HideRule>) -> Self {
        Self {
            display_mode,
            hide_rules,
        }
    }
    pub(crate) fn select_monitor_for_rect<'a>(
        &self,
        monitors: &'a [MonitorSnapshot],
        rect: PhysicalRect,
    ) -> Option<&'a MonitorSnapshot> {
        monitors.iter().max_by_key(|monitor| {
            let overlap = monitor.monitor_area_physical.overlap_area(rect);
            let center = i64::from(monitor.monitor_area_physical.contains_center(rect));
            let primary = i64::from(monitor.is_primary);
            (overlap, center, primary)
        })
    }

    pub(crate) fn physical_to_logical(
        &self,
        monitor: &MonitorSnapshot,
        x: i32,
        y: i32,
    ) -> (f64, f64) {
        let scale = monitor.scale_factor.max(1.0);
        (
            f64::from(x - monitor.work_area_physical.left) / scale,
            f64::from(y - monitor.work_area_physical.top) / scale,
        )
    }

    pub(crate) fn logical_to_physical(
        &self,
        monitor: &MonitorSnapshot,
        x: f64,
        y: f64,
    ) -> (i32, i32) {
        let scale = monitor.scale_factor.max(1.0);
        (
            monitor.work_area_physical.left + (x * scale).round() as i32,
            monitor.work_area_physical.top + (y * scale).round() as i32,
        )
    }

    pub(crate) fn resolve(
        &self,
        snapshot: &EnvironmentSnapshot,
        pet_size_logical: f64,
        recovery: Option<&RecoveryPosition>,
        manually_hidden: bool,
    ) -> EnvironmentDecision {
        let Some(monitor) = self.select_target_monitor(snapshot, recovery) else {
            return EnvironmentDecision {
                monitor_id: None,
                footing: None,
                visibility_reason: Some(VisibilityReason::MonitorUnavailable),
                manually_hidden,
            };
        };
        let foreground = snapshot
            .foreground
            .as_ref()
            .filter(|window| window.is_normal());
        let visibility_reason =
            if foreground.is_some_and(|window| self.is_fullscreen(snapshot, window)) {
                Some(VisibilityReason::Fullscreen)
            } else if self
                .hide_rules
                .iter()
                .any(|rule| rule.matches(foreground.and_then(|window| window.app_id.as_deref())))
            {
                Some(VisibilityReason::SpecifiedApp)
            } else if self.display_mode == DisplayMode::DesktopOnly && foreground.is_some() {
                Some(VisibilityReason::DesktopOnlyForeground)
            } else {
                None
            };
        let footing = if self.display_mode == DisplayMode::AboveNormalWindows {
            foreground
                .and_then(|window| self.foreground_footing(monitor, window, pet_size_logical))
                .unwrap_or_else(|| self.desktop_footing(monitor, pet_size_logical))
        } else {
            self.desktop_footing(monitor, pet_size_logical)
        };
        EnvironmentDecision {
            monitor_id: Some(monitor.id.clone()),
            footing: Some(footing),
            visibility_reason,
            manually_hidden,
        }
    }

    fn is_fullscreen(
        &self,
        snapshot: &EnvironmentSnapshot,
        window: &ForegroundWindowSnapshot,
    ) -> bool {
        if window.is_fullscreen {
            return true;
        }
        let Some(monitor) = self.select_monitor_for_rect(&snapshot.monitors, window.rect_physical)
        else {
            return false;
        };
        let bounds = monitor.monitor_area_physical;
        const TOLERANCE: i32 = 2;
        window.rect_physical.left <= bounds.left + TOLERANCE
            && window.rect_physical.top <= bounds.top + TOLERANCE
            && window.rect_physical.right >= bounds.right - TOLERANCE
            && window.rect_physical.bottom >= bounds.bottom - TOLERANCE
    }

    fn select_target_monitor<'a>(
        &self,
        snapshot: &'a EnvironmentSnapshot,
        recovery: Option<&RecoveryPosition>,
    ) -> Option<&'a MonitorSnapshot> {
        if self.display_mode == DisplayMode::AboveNormalWindows {
            if let Some(window) = snapshot
                .foreground
                .as_ref()
                .filter(|window| window.is_normal())
            {
                if let Some(monitor) =
                    self.select_monitor_for_rect(&snapshot.monitors, window.rect_physical)
                {
                    return Some(monitor);
                }
            }
        }
        recovery
            .and_then(|position| position.monitor_id.as_deref())
            .and_then(|id| snapshot.monitors.iter().find(|monitor| monitor.id == id))
            .or_else(|| snapshot.monitors.iter().find(|monitor| monitor.is_primary))
            .or_else(|| snapshot.monitors.first())
    }

    fn desktop_footing(&self, monitor: &MonitorSnapshot, pet_size_logical: f64) -> Footing {
        let width = monitor.logical_width();
        let height = monitor.logical_height();
        Footing {
            id: format!("{}:desktop-work-area", monitor.id),
            monitor_id: monitor.id.clone(),
            top_y_logical: (height - pet_size_logical).max(0.0),
            min_x_logical: 0.0,
            max_x_logical: (width - pet_size_logical).max(0.0),
            source: FootingSource::DesktopWorkArea,
        }
    }

    fn foreground_footing(
        &self,
        monitor: &MonitorSnapshot,
        window: &ForegroundWindowSnapshot,
        pet_size_logical: f64,
    ) -> Option<Footing> {
        let pet_size = (pet_size_logical * monitor.scale_factor.max(1.0)).round() as i32;
        let work = monitor.work_area_physical;
        let min_x = window.rect_physical.left.max(work.left);
        let max_x = (window.rect_physical.right - pet_size).min(work.right - pet_size);
        if min_x > max_x {
            return None;
        }
        let top = (window.rect_physical.top - pet_size).clamp(work.top, work.bottom - pet_size);
        let (min_x_logical, top_y_logical) = self.physical_to_logical(monitor, min_x, top);
        let (max_x_logical, _) = self.physical_to_logical(monitor, max_x, top);
        Some(Footing {
            id: format!("{}:foreground-window-top", monitor.id),
            monitor_id: monitor.id.clone(),
            top_y_logical,
            min_x_logical,
            max_x_logical,
            source: FootingSource::ForegroundWindowTop,
        })
    }
}

impl Default for EnvironmentPolicy {
    fn default() -> Self {
        Self::new(DisplayMode::AboveNormalWindows, vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentPolicy, EnvironmentPort, MonitorSnapshot, PhysicalRect, WindowPort};

    struct FakeEnvironmentPort(super::EnvironmentSnapshot);
    impl EnvironmentPort for FakeEnvironmentPort {
        fn snapshot(&self, _captured_at_ms: u64) -> super::EnvironmentSnapshot {
            self.0.clone()
        }
    }

    #[derive(Default)]
    struct FakeWindowPort(std::cell::Cell<bool>);
    impl WindowPort for FakeWindowPort {
        fn set_visible(&self, visible: bool) -> Result<(), String> {
            self.0.set(visible);
            Ok(())
        }
        fn place(&self, _x_physical: i32, _y_physical: i32) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn m2_selects_the_foreground_windows_largest_overlapping_monitor_and_converts_dpi() {
        let policy = EnvironmentPolicy::default();
        let monitors = vec![
            MonitorSnapshot::new("left", PhysicalRect::new(-1920, 0, 0, 1080), 1.0, false),
            MonitorSnapshot::new("primary", PhysicalRect::new(0, 0, 3840, 2160), 1.5, true),
        ];

        let selected =
            policy.select_monitor_for_rect(&monitors, PhysicalRect::new(-100, 100, 1200, 900));

        assert_eq!(selected.expect("monitor").id, "primary");
        assert_eq!(
            policy.physical_to_logical(&monitors[1], 150, 300),
            (100.0, 200.0)
        );
    }

    #[test]
    fn m2_resolves_foreground_footing_and_visibility_priority_without_win32() {
        let monitor =
            MonitorSnapshot::new("primary", PhysicalRect::new(0, 0, 1920, 1040), 1.0, true);
        let snapshot = super::EnvironmentSnapshot {
            monitors: vec![monitor.clone()],
            foreground: Some(super::ForegroundWindowSnapshot::normal(
                Some("editor.exe".into()),
                PhysicalRect::new(120, 240, 1_120, 900),
                false,
            )),
            captured_at_ms: 1,
        };
        let policy = super::EnvironmentPolicy::new(super::DisplayMode::AboveNormalWindows, vec![]);
        let decision = policy.resolve(&snapshot, 180.0, None, false);
        assert_eq!(decision.visibility_reason, None);
        assert_eq!(
            decision.footing.expect("footing").source,
            crate::model::FootingSource::ForegroundWindowTop
        );

        let fullscreen = super::EnvironmentSnapshot {
            foreground: Some(super::ForegroundWindowSnapshot::normal(
                Some("editor.exe".into()),
                PhysicalRect::new(0, 0, 1920, 1040),
                true,
            )),
            ..snapshot
        };
        assert_eq!(
            policy
                .resolve(&fullscreen, 180.0, None, false)
                .visibility_reason,
            Some(super::VisibilityReason::Fullscreen)
        );
    }

    #[test]
    fn m2_applies_hide_modes_and_recovers_after_monitor_disconnect() {
        let left = MonitorSnapshot::new("left", PhysicalRect::new(-1920, 0, 0, 1080), 1.0, false);
        let primary =
            MonitorSnapshot::new("primary", PhysicalRect::new(0, 0, 2880, 1560), 1.5, true);
        let window = super::ForegroundWindowSnapshot::normal(
            Some("C:/Apps/Editor.EXE".into()),
            PhysicalRect::new(-1800, 200, -600, 900),
            false,
        );
        let snapshot = super::EnvironmentSnapshot {
            monitors: vec![left.clone(), primary.clone()],
            foreground: Some(window),
            captured_at_ms: 2,
        };
        let policy = super::EnvironmentPolicy::new(
            super::DisplayMode::AboveNormalWindows,
            vec![super::HideRule {
                app_id: "C:/Apps/Editor.EXE".into(),
            }],
        );
        assert_eq!(
            policy
                .resolve(&snapshot, 180.0, None, false)
                .visibility_reason,
            Some(super::VisibilityReason::SpecifiedApp)
        );

        let desktop_only = super::EnvironmentPolicy::new(super::DisplayMode::DesktopOnly, vec![]);
        assert_eq!(
            desktop_only
                .resolve(&snapshot, 180.0, None, false)
                .visibility_reason,
            Some(super::VisibilityReason::DesktopOnlyForeground)
        );

        let disconnected = super::EnvironmentSnapshot {
            monitors: vec![primary],
            foreground: None,
            captured_at_ms: 3,
        };
        let restored = policy.resolve(
            &disconnected,
            180.0,
            Some(&super::RecoveryPosition {
                monitor_id: Some("left".into()),
                normalized_x: 0.7,
            }),
            false,
        );
        assert_eq!(restored.monitor_id.as_deref(), Some("primary"));
        assert_eq!(
            restored.footing.expect("recovery footing").source,
            crate::model::FootingSource::DesktopWorkArea
        );
    }

    #[test]
    fn m2_policy_is_testable_through_fake_environment_and_window_ports() {
        let snapshot = super::EnvironmentSnapshot {
            monitors: vec![MonitorSnapshot::new(
                "primary",
                PhysicalRect::new(0, 0, 1920, 1040),
                1.0,
                true,
            )],
            foreground: None,
            captured_at_ms: 0,
        };
        let environment = FakeEnvironmentPort(snapshot);
        let window = FakeWindowPort::default();
        let decision =
            EnvironmentPolicy::default().resolve(&environment.snapshot(10), 180.0, None, false);
        window
            .set_visible(decision.is_visible())
            .expect("fake window accepts policy");
        window.place(0, 0).expect("fake window accepts placement");
        assert!(window.0.get());
    }
}
