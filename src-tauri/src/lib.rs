mod behavior;
mod environment;
mod model;
mod platform;
mod settings;

use behavior::{BehaviorPlanner, Lcg, PlannerConfig};
use environment::{
    DisplayMode, EnvironmentPolicy, EnvironmentPort, RecoveryPosition, VisibilityReason,
};
use model::{
    AnimationObservation, Footing, FootingSource, InputObservation, PetScale, PetSettings,
    RuntimeSnapshot, SettingsPatch, TutorialBubbleDirective, VisibilityPayload, WorldPoint,
    PROTOCOL_VERSION,
};
use settings::SettingsStore;
use std::{
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, State, WebviewWindow,
};

#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

const PET_WINDOW: &str = "pet";
const BUBBLE_WINDOW: &str = "bubble";
const BUBBLE_WIDTH: u32 = 260;
const BUBBLE_HEIGHT: u32 = 96;

struct CoreState {
    store: SettingsStore,
    runtime: Mutex<RuntimeData>,
}

struct RuntimeData {
    settings: PetSettings,
    manually_hidden: bool,
    visibility_reason: Option<VisibilityReason>,
    environment_policy: EnvironmentPolicy,
    drag: Option<DragState>,
    planner: BehaviorPlanner<Lcg>,
    tutorial_bubble: TutorialBubbleDirective,
    next_tutorial_bubble_id: u64,
    next_tutorial_bubble_sequence: u64,
}

#[derive(Clone, Copy)]
struct DragState {
    offset_x: f64,
    offset_y: f64,
}

struct MenuHandles {
    menu: Menu<tauri::Wry>,
    visible: CheckMenuItem<tauri::Wry>,
    sound: CheckMenuItem<tauri::Wry>,
    autostart: CheckMenuItem<tauri::Wry>,
    small: CheckMenuItem<tauri::Wry>,
    medium: CheckMenuItem<tauri::Wry>,
    large: CheckMenuItem<tauri::Wry>,
}

#[derive(Clone)]
struct MonitorArea {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f64,
}

struct PlannerPlacement {
    area: MonitorArea,
    pet_size_physical: u32,
    footing: Footing,
    anchor: WorldPoint,
}

impl MonitorArea {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x < f64::from(self.x) + f64::from(self.width)
            && y >= f64::from(self.y)
            && y < f64::from(self.y) + f64::from(self.height)
    }
}

fn lock_runtime(state: &CoreState) -> std::sync::MutexGuard<'_, RuntimeData> {
    state
        .runtime
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn effective_visible(data: &RuntimeData) -> bool {
    !data.manually_hidden && data.visibility_reason.is_none()
}

fn visibility_payload(data: &RuntimeData) -> VisibilityPayload {
    VisibilityPayload {
        visible: effective_visible(data),
        manually_hidden: data.manually_hidden,
        fullscreen_hidden: data.visibility_reason == Some(VisibilityReason::Fullscreen),
    }
}

fn emit_visibility<R: Runtime>(app: &AppHandle<R>, data: &RuntimeData) {
    let payload = visibility_payload(data);
    let _ = app.emit("pet://visibility", payload);
}

fn apply_window_visibility<R: Runtime>(app: &AppHandle<R>, data: &RuntimeData) {
    let visible = effective_visible(data);
    if let Some(pet) = app.get_webview_window(PET_WINDOW) {
        if visible {
            let _ = pet.show();
        } else {
            let _ = pet.hide();
        }
    }
    if let Some(bubble) = app.get_webview_window(BUBBLE_WINDOW) {
        if !visible {
            let _ = bubble.hide();
        } else if data.tutorial_bubble.visible {
            let _ = bubble.show();
        } else {
            let _ = bubble.hide();
        }
    }
    emit_visibility(app, data);
}

fn save_settings(state: &CoreState, settings: &PetSettings) -> Result<(), String> {
    state
        .store
        .save(settings)
        .map_err(|error| error.to_string())
}

fn emit_settings<R: Runtime>(app: &AppHandle<R>, settings: &PetSettings) {
    let _ = app.emit("pet://settings", settings.clone());
}

fn tutorial_text(step: u8) -> Option<&'static str> {
    ["点我一下？", "还能拖我走！", "右键能管住我。"]
        .get(usize::from(step))
        .copied()
}

fn tutorial_directive(step: u8, id: u64, sequence: u64) -> TutorialBubbleDirective {
    TutorialBubbleDirective {
        protocol_version: PROTOCOL_VERSION,
        sequence,
        id,
        visible: tutorial_text(step).is_some(),
        text: tutorial_text(step).map(str::to_owned),
    }
}

fn replace_tutorial_directive(data: &mut RuntimeData) {
    data.tutorial_bubble = tutorial_directive(
        data.settings.tutorial_step,
        data.next_tutorial_bubble_id,
        data.next_tutorial_bubble_sequence,
    );
    data.next_tutorial_bubble_id = data.next_tutorial_bubble_id.saturating_add(1);
    data.next_tutorial_bubble_sequence = data.next_tutorial_bubble_sequence.saturating_add(1);
}

fn emit_tutorial_bubble<R: Runtime>(app: &AppHandle<R>, data: &RuntimeData) {
    debug_assert!(data.tutorial_bubble.validate().is_ok());
    let _ = app.emit(
        "pet://tutorial-bubble-directive",
        data.tutorial_bubble.clone(),
    );
    if let Some(window) = app.get_webview_window(BUBBLE_WINDOW) {
        if effective_visible(data) && data.tutorial_bubble.visible {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }
}

fn advance_tutorial_if_expected<R: Runtime>(
    app: &AppHandle<R>,
    state: &CoreState,
    data: &mut RuntimeData,
    expected_step: u8,
    next_step: u8,
) -> Result<(), String> {
    if data.settings.tutorial_step != expected_step {
        return Ok(());
    }
    data.settings.tutorial_step = next_step;
    replace_tutorial_directive(data);
    save_settings(state, &data.settings)?;
    emit_settings(app, &data.settings);
    emit_tutorial_bubble(app, data);
    Ok(())
}

fn emit_m1_snapshot<R: Runtime>(app: &AppHandle<R>, data: &mut RuntimeData, now_ms: u64) {
    let mut snapshot = data.planner.runtime_snapshot(now_ms);
    snapshot.display_mode = data.environment_policy.display_mode;
    snapshot.manually_hidden = data.manually_hidden;
    snapshot.visibility_reason = data.visibility_reason;
    debug_assert!(snapshot.validate().is_ok());
    let _ = app.emit("pet://runtime-snapshot", snapshot);
}

fn emit_m1_plan<R: Runtime>(app: &AppHandle<R>, data: &mut RuntimeData, now_ms: u64) {
    let _ = app.emit("pet://motion-plan", data.planner.active_plan().clone());
    emit_m1_snapshot(app, data, now_ms);
}

fn monitor_areas<R: Runtime>(window: &WebviewWindow<R>) -> Result<Vec<MonitorArea>, String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let work = monitor.work_area();
            MonitorArea {
                id: monitor
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("monitor-{index}")),
                x: work.position.x,
                y: work.position.y,
                width: work.size.width,
                height: work.size.height,
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect())
}

fn selected_monitor<R: Runtime>(
    window: &WebviewWindow<R>,
    requested_id: Option<&str>,
) -> Result<MonitorArea, String> {
    let areas = monitor_areas(window)?;
    if let Some(id) = requested_id {
        if let Some(area) = areas.iter().find(|area| area.id == id) {
            return Ok(area.clone());
        }
    }

    if let Some(primary) = window
        .primary_monitor()
        .map_err(|error| error.to_string())?
    {
        let work = primary.work_area();
        let primary_name = primary.name();
        if let Some(area) = areas.iter().find(|area| {
            primary_name.is_some_and(|name| &area.id == name)
                || (area.x == work.position.x && area.y == work.position.y)
        }) {
            return Ok(area.clone());
        }
    }
    areas
        .into_iter()
        .next()
        .ok_or_else(|| "没有检测到可用显示器".to_string())
}

fn physical_pet_size(scale: PetScale, monitor: &MonitorArea) -> u32 {
    (f64::from(scale.logical_pixels()) * monitor.scale_factor)
        .round()
        .max(1.0) as u32
}

fn x_from_normalized(area: &MonitorArea, pet_width: u32, normalized_x: f64) -> i32 {
    let travel = area.width.saturating_sub(pet_width);
    area.x + (f64::from(travel) * normalized_x.clamp(0.0, 1.0)).round() as i32
}

fn normalized_from_x(area: &MonitorArea, pet_width: u32, x: i32) -> f64 {
    let travel = area.width.saturating_sub(pet_width);
    if travel == 0 {
        0.0
    } else {
        (f64::from(x - area.x) / f64::from(travel)).clamp(0.0, 1.0)
    }
}

fn m1_footing(area: &MonitorArea, pet_size: u32) -> Footing {
    let scale = area.scale_factor.max(1.0);
    Footing {
        id: format!("{}:desktop-work-area", area.id),
        monitor_id: area.id.clone(),
        top_y_logical: (f64::from(area.height.saturating_sub(pet_size))) / scale,
        min_x_logical: 0.0,
        max_x_logical: f64::from(area.width.saturating_sub(pet_size)) / scale,
        source: FootingSource::DesktopWorkArea,
    }
}

fn m1_world_point(area: &MonitorArea, pet_size: u32, x: i32, y: i32) -> WorldPoint {
    let scale = area.scale_factor.max(1.0);
    WorldPoint::new(
        area.id.clone(),
        f64::from(x - area.x) / scale,
        (f64::from(y - area.y)).min(f64::from(area.height.saturating_sub(pet_size))) / scale,
    )
}

fn place_bubble<R: Runtime>(app: &AppHandle<R>, pet_x: i32, pet_y: i32, area: &MonitorArea) {
    let Some(bubble) = app.get_webview_window(BUBBLE_WINDOW) else {
        return;
    };
    let scale = area.scale_factor;
    let width = (f64::from(BUBBLE_WIDTH) * scale).round() as i32;
    let height = (f64::from(BUBBLE_HEIGHT) * scale).round() as i32;
    let max_x = area.x + area.width as i32 - width;
    let x = (pet_x - width / 3).clamp(area.x, max_x.max(area.x));
    let y = (pet_y - height).max(area.y);
    let _ = bubble.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
    let _ = bubble.set_position(PhysicalPosition::new(x, y));
}

fn planner_placement(settings: &PetSettings, area: MonitorArea) -> PlannerPlacement {
    let pet_size_physical = physical_pet_size(settings.scale, &area);
    let footing = m1_footing(&area, pet_size_physical);
    let x = x_from_normalized(&area, pet_size_physical, settings.normalized_x);
    let anchor = m1_world_point(
        &area,
        pet_size_physical,
        x,
        area.y + area.height as i32 - pet_size_physical as i32,
    );
    PlannerPlacement {
        area,
        pet_size_physical,
        footing,
        anchor,
    }
}

fn reanchor_from_settings<R: Runtime>(
    app: &AppHandle<R>,
    data: &mut RuntimeData,
    now_ms: u64,
) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let area = selected_monitor(&pet, data.settings.monitor_id.as_deref())?;
    let placement = planner_placement(&data.settings, area);
    pet.set_size(PhysicalSize::new(
        placement.pet_size_physical,
        placement.pet_size_physical,
    ))
    .map_err(|error| error.to_string())?;
    data.settings.monitor_id = Some(placement.area.id.clone());
    data.planner
        .reanchor(now_ms, placement.footing, placement.anchor);
    apply_m1_position(app, &data.planner.position_at(now_ms));
    emit_m1_plan(app, data, now_ms);
    Ok(())
}

fn sync_menu<R: Runtime>(app: &AppHandle<R>, data: &RuntimeData) {
    let Some(handles) = app.try_state::<MenuHandles>() else {
        return;
    };
    let _ = handles.visible.set_checked(!data.manually_hidden);
    let _ = handles.sound.set_checked(data.settings.sound_enabled);
    let _ = handles
        .autostart
        .set_checked(data.settings.autostart_enabled);
    let _ = handles
        .small
        .set_checked(data.settings.scale == PetScale::Small);
    let _ = handles
        .medium
        .set_checked(data.settings.scale == PetScale::Medium);
    let _ = handles
        .large
        .set_checked(data.settings.scale == PetScale::Large);
}

#[tauri::command]
fn get_settings(state: State<'_, CoreState>) -> PetSettings {
    lock_runtime(&state).settings.clone()
}

#[tauri::command]
fn get_runtime_snapshot(state: State<'_, CoreState>) -> RuntimeSnapshot {
    let mut data = lock_runtime(&state);
    let mut snapshot = data.planner.runtime_snapshot(current_ms());
    snapshot.display_mode = data.environment_policy.display_mode;
    snapshot.manually_hidden = data.manually_hidden;
    snapshot.visibility_reason = data.visibility_reason;
    snapshot
}

#[tauri::command]
fn get_tutorial_bubble_directive(state: State<'_, CoreState>) -> TutorialBubbleDirective {
    lock_runtime(&state).tutorial_bubble.clone()
}

#[tauri::command]
fn update_settings(
    app: AppHandle,
    state: State<'_, CoreState>,
    patch: SettingsPatch,
) -> Result<PetSettings, String> {
    let requested_autostart = patch.autostart_enabled;
    if let Some(enabled) = requested_autostart {
        set_os_autostart(&app, enabled)?;
    }

    let mut data = lock_runtime(&state);
    patch.apply(&mut data.settings);
    reanchor_from_settings(&app, &mut data, current_ms())?;
    save_settings(&state, &data.settings)?;
    sync_menu(&app, &data);
    emit_settings(&app, &data.settings);
    Ok(data.settings.clone())
}

#[tauri::command]
fn set_pet_visible(
    app: AppHandle,
    state: State<'_, CoreState>,
    visible: bool,
) -> Result<VisibilityPayload, String> {
    let mut data = lock_runtime(&state);
    data.manually_hidden = !visible;
    apply_window_visibility(&app, &data);
    emit_m1_snapshot(&app, &mut data, current_ms());
    sync_menu(&app, &data);
    Ok(visibility_payload(&data))
}

/// M2 developer control only: intentionally in-memory, with no M4 settings UI
/// or persistence surface.
#[tauri::command]
fn set_m2_environment_policy(
    app: AppHandle,
    state: State<'_, CoreState>,
    display_mode: DisplayMode,
    hide_rules: Vec<environment::HideRule>,
) -> Result<(), String> {
    if hide_rules.iter().any(|rule| rule.app_id.trim().is_empty()) {
        return Err("隐藏规则的应用标识不能为空".into());
    }
    {
        let mut data = lock_runtime(&state);
        data.environment_policy = EnvironmentPolicy::new(display_mode, hide_rules);
    }
    update_windows_environment(&app);
    Ok(())
}

#[tauri::command]
fn set_pet_scale(
    app: AppHandle,
    state: State<'_, CoreState>,
    scale: PetScale,
) -> Result<PetSettings, String> {
    update_settings(
        app,
        state,
        SettingsPatch {
            scale: Some(scale),
            ..Default::default()
        },
    )
}

#[tauri::command]
fn set_sound_enabled(
    app: AppHandle,
    state: State<'_, CoreState>,
    enabled: bool,
) -> Result<PetSettings, String> {
    update_settings(
        app,
        state,
        SettingsPatch {
            sound_enabled: Some(enabled),
            ..Default::default()
        },
    )
}

fn set_os_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        if enabled {
            app.autolaunch().enable().map_err(|error| error.to_string())
        } else {
            app.autolaunch()
                .disable()
                .map_err(|error| error.to_string())
        }
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, enabled);
        Err("此平台不支持开机启动".into())
    }
}

#[tauri::command]
fn set_autostart_enabled(
    app: AppHandle,
    state: State<'_, CoreState>,
    enabled: bool,
) -> Result<PetSettings, String> {
    update_settings(
        app,
        state,
        SettingsPatch {
            autostart_enabled: Some(enabled),
            ..Default::default()
        },
    )
}

#[tauri::command]
fn reset_pet_position(app: AppHandle, state: State<'_, CoreState>) -> Result<PetSettings, String> {
    let mut data = lock_runtime(&state);
    data.settings.monitor_id = None;
    data.settings.normalized_x = PetSettings::default().normalized_x;
    reanchor_from_settings(&app, &mut data, current_ms())?;
    save_settings(&state, &data.settings)?;
    emit_settings(&app, &data.settings);
    Ok(data.settings.clone())
}

#[tauri::command]
fn begin_drag(
    app: AppHandle,
    state: State<'_, CoreState>,
    pointer_x: f64,
    pointer_y: f64,
) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let position = pet.outer_position().map_err(|error| error.to_string())?;
    let cursor = pet
        .cursor_position()
        .unwrap_or_else(|_| PhysicalPosition::new(pointer_x, pointer_y));
    let mut data = lock_runtime(&state);
    data.drag = Some(DragState {
        offset_x: cursor.x - f64::from(position.x),
        offset_y: cursor.y - f64::from(position.y),
    });
    let now_ms = current_ms();
    data.planner.begin_drag(now_ms);
    emit_m1_plan(&app, &mut data, now_ms);
    Ok(())
}

#[tauri::command]
fn drag_pet(
    app: AppHandle,
    state: State<'_, CoreState>,
    pointer_x: f64,
    pointer_y: f64,
) -> Result<(), String> {
    let (drag, footing) = {
        let data = lock_runtime(&state);
        (
            data.drag.ok_or_else(|| "尚未开始拖动".to_string())?,
            data.planner.footing().clone(),
        )
    };
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let cursor = pet
        .cursor_position()
        .unwrap_or_else(|_| PhysicalPosition::new(pointer_x, pointer_y));
    let x = (cursor.x - drag.offset_x).round() as i32;
    let y = (cursor.y - drag.offset_y).round() as i32;
    let size = pet.outer_size().map_err(|error| error.to_string())?;
    let areas = monitor_areas(&pet)?;
    let area = areas
        .iter()
        .find(|area| area.id == footing.monitor_id)
        .ok_or_else(|| "权威落脚显示器不可用".to_string())?;
    let point = m1_world_point(area, size.width, x, y);
    let now_ms = current_ms();
    let mut data = lock_runtime(&state);
    data.planner.drag_to(now_ms, point.clone());
    apply_m1_position(&app, &point);
    emit_m1_plan(&app, &mut data, now_ms);
    Ok(())
}

#[tauri::command]
fn end_drag(
    app: AppHandle,
    state: State<'_, CoreState>,
    pointer_x: f64,
    pointer_y: f64,
) -> Result<PetSettings, String> {
    drag_pet(app.clone(), state.clone(), pointer_x, pointer_y)?;
    let mut data = lock_runtime(&state);
    data.drag = None;
    let now_ms = current_ms();
    let plan = data.planner.land_after_drag(now_ms);
    let footing = data.planner.footing().clone();
    data.settings.normalized_x = if footing.max_x_logical > footing.min_x_logical {
        ((plan.to.x_logical - footing.min_x_logical)
            / (footing.max_x_logical - footing.min_x_logical))
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    data.settings.monitor_id = Some(footing.monitor_id);
    emit_m1_plan(&app, &mut data, now_ms);
    let advanced_tutorial = data.settings.tutorial_step == 1;
    if advanced_tutorial {
        data.settings.tutorial_step = 2;
        replace_tutorial_directive(&mut data);
    }
    save_settings(&state, &data.settings)?;
    emit_settings(&app, &data.settings);
    if advanced_tutorial {
        emit_tutorial_bubble(&app, &data);
    }
    Ok(data.settings.clone())
}

#[tauri::command]
fn input_observed(
    app: AppHandle,
    state: State<'_, CoreState>,
    protocol_version: u8,
    observation: InputObservation,
) -> Result<(), String> {
    if protocol_version != PROTOCOL_VERSION {
        eprintln!("[protocol] ignored unsupported input version {protocol_version}");
        return Ok(());
    }
    match observation {
        InputObservation::SingleClick => {
            let mut data = lock_runtime(&state);
            advance_tutorial_if_expected(&app, &state, &mut data, 0, 1)
        }
        InputObservation::DragStarted {
            pointer_x_physical,
            pointer_y_physical,
        } => begin_drag(app, state, pointer_x_physical, pointer_y_physical),
        InputObservation::DragMoved {
            pointer_x_physical,
            pointer_y_physical,
        } => drag_pet(app, state, pointer_x_physical, pointer_y_physical),
        InputObservation::DragEnded {
            pointer_x_physical,
            pointer_y_physical,
        } => end_drag(app, state, pointer_x_physical, pointer_y_physical).map(|_| ()),
    }
}

#[tauri::command]
fn animation_observed(state: State<'_, CoreState>, observation: AnimationObservation) {
    if observation.protocol_version != PROTOCOL_VERSION {
        eprintln!("[protocol] ignored unsupported animation version");
        return;
    }
    let data = lock_runtime(&state);
    if data.planner.active_plan().id != observation.plan_id {
        eprintln!(
            "[m1 protocol] ignored stale animation for plan {}",
            observation.plan_id
        );
    }
}

#[tauri::command]
fn show_context_menu(
    app: AppHandle,
    state: State<'_, CoreState>,
    menus: State<'_, MenuHandles>,
) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    pet.popup_menu(&menus.menu)
        .map_err(|error| error.to_string())?;
    let mut data = lock_runtime(&state);
    advance_tutorial_if_expected(&app, &state, &mut data, 2, 3)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "visible" => {
            let state = app.state::<CoreState>();
            let visible = lock_runtime(&state).manually_hidden;
            let _ = set_pet_visible(app.clone(), state, visible);
        }
        "sound" => {
            let state = app.state::<CoreState>();
            let enabled = !lock_runtime(&state).settings.sound_enabled;
            let _ = set_sound_enabled(app.clone(), state, enabled);
        }
        "autostart" => {
            let state = app.state::<CoreState>();
            let enabled = !lock_runtime(&state).settings.autostart_enabled;
            let _ = set_autostart_enabled(app.clone(), state, enabled);
        }
        "scale-small" => {
            let _ = set_pet_scale(app.clone(), app.state(), PetScale::Small);
        }
        "scale-medium" => {
            let _ = set_pet_scale(app.clone(), app.state(), PetScale::Medium);
        }
        "scale-large" => {
            let _ = set_pet_scale(app.clone(), app.state(), PetScale::Large);
        }
        "reset-position" => {
            let _ = reset_pet_position(app.clone(), app.state());
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn create_menu(app: &tauri::App) -> tauri::Result<MenuHandles> {
    let settings = lock_runtime(&app.state::<CoreState>()).settings.clone();
    let visible = CheckMenuItem::with_id(app, "visible", "显示宠物", true, true, None::<&str>)?;
    let sound = CheckMenuItem::with_id(
        app,
        "sound",
        "声音",
        true,
        settings.sound_enabled,
        None::<&str>,
    )?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机启动",
        true,
        settings.autostart_enabled,
        None::<&str>,
    )?;
    let small = CheckMenuItem::with_id(
        app,
        "scale-small",
        "小（120px）",
        true,
        settings.scale == PetScale::Small,
        None::<&str>,
    )?;
    let medium = CheckMenuItem::with_id(
        app,
        "scale-medium",
        "中（180px）",
        true,
        settings.scale == PetScale::Medium,
        None::<&str>,
    )?;
    let large = CheckMenuItem::with_id(
        app,
        "scale-large",
        "大（260px）",
        true,
        settings.scale == PetScale::Large,
        None::<&str>,
    )?;
    let menu = MenuBuilder::new(app)
        .item(&visible)
        .separator()
        .item(&small)
        .item(&medium)
        .item(&large)
        .separator()
        .item(&sound)
        .item(&autostart)
        .separator()
        .text("reset-position", "重置位置")
        .text("quit", "退出")
        .build()?;
    Ok(MenuHandles {
        menu,
        visible,
        sound,
        autostart,
        small,
        medium,
        large,
    })
}

fn configure_windows(app: &tauri::App) -> tauri::Result<()> {
    if let Some(pet) = app.get_webview_window(PET_WINDOW) {
        pet.set_always_on_top(true)?;
        pet.set_skip_taskbar(true)?;
        pet.set_shadow(false)?;
    }
    if let Some(bubble) = app.get_webview_window(BUBBLE_WINDOW) {
        bubble.set_always_on_top(true)?;
        bubble.set_skip_taskbar(true)?;
        bubble.set_shadow(false)?;
        bubble.set_ignore_cursor_events(true)?;
        bubble.set_focusable(false)?;
        bubble.hide()?;
    }
    Ok(())
}

fn start_runtime(app: AppHandle) {
    thread::spawn(move || {
        let mut environment_check_due = 0_u8;
        loop {
            let state = app.state::<CoreState>();
            let visible = {
                let data = lock_runtime(&state);
                effective_visible(&data)
            };

            let sleep_ms = if visible { 33 } else { 250 };
            thread::sleep(Duration::from_millis(sleep_ms));

            environment_check_due = environment_check_due.saturating_add(1);
            let check_every = if visible { 30 } else { 4 };
            if environment_check_due >= check_every {
                environment_check_due = 0;
                update_windows_environment(&app);
            }

            let mut data = lock_runtime(&state);
            if !effective_visible(&data) {
                continue;
            }
            let now_ms = current_ms();
            if data.planner.tick(now_ms).is_some() {
                emit_m1_plan(&app, &mut data, now_ms);
            }
            if data.planner.behavior() != model::BehaviorState::Dragged {
                apply_m1_position(&app, &data.planner.position_at(now_ms));
            }
        }
    });
}

fn apply_m1_position<R: Runtime>(app: &AppHandle<R>, point: &WorldPoint) {
    let Some(pet) = app.get_webview_window(PET_WINDOW) else {
        return;
    };
    let Ok(areas) = monitor_areas(&pet) else {
        return;
    };
    let Some(area) = areas.iter().find(|area| area.id == point.monitor_id) else {
        return;
    };
    let x = area.x + (point.x_logical * area.scale_factor).round() as i32;
    let y = area.y + (point.y_logical * area.scale_factor).round() as i32;
    let _ = pet.set_position(PhysicalPosition::new(x, y));
    place_bubble(app, x, y, area);
}

fn update_windows_environment(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    let ignored = [PET_WINDOW, BUBBLE_WINDOW]
        .iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter_map(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
        .collect::<Vec<_>>();
    #[cfg(not(target_os = "windows"))]
    let ignored = Vec::new();

    let state = app.state::<CoreState>();
    let mut data = lock_runtime(&state);
    let snapshot = platform::WindowsEnvironmentPort::new(ignored).snapshot(current_ms());
    let recovery = RecoveryPosition {
        monitor_id: data.settings.monitor_id.clone(),
        normalized_x: data.settings.normalized_x,
    };
    let decision = data.environment_policy.resolve(
        &snapshot,
        f64::from(data.settings.scale.logical_pixels()),
        Some(&recovery),
        data.manually_hidden,
    );
    let changed_visibility = data.visibility_reason != decision.visibility_reason;
    data.visibility_reason = decision.visibility_reason;
    if let Some(footing) = decision.footing {
        if *data.planner.footing() != footing
            && data.planner.behavior() != model::BehaviorState::Dragged
        {
            let x = if data.planner.position().monitor_id == footing.monitor_id {
                data.planner.position_at(current_ms()).x_logical
            } else {
                footing.min_x_logical
                    + (footing.max_x_logical - footing.min_x_logical)
                        * recovery.normalized_x.clamp(0.0, 1.0)
            };
            let point = WorldPoint::new(footing.monitor_id.clone(), x, footing.top_y_logical);
            data.planner.reanchor(current_ms(), footing, point);
            emit_m1_plan(app, &mut data, current_ms());
        } else if changed_visibility {
            emit_m1_snapshot(app, &mut data, current_ms());
        }
    } else if changed_visibility {
        emit_m1_snapshot(app, &mut data, current_ms());
    }
    if changed_visibility {
        apply_window_visibility(app, &data);
    }
}

fn current_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1)
}

fn current_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let state = app.state::<CoreState>();
            let mut data = lock_runtime(&state);
            data.manually_hidden = false;
            apply_window_visibility(app, &data);
            sync_menu(app, &data);
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let store = SettingsStore::new(app_data);
            let mut settings = store.load()?;
            #[cfg(desktop)]
            {
                settings.autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
            }
            configure_windows(app)?;
            let pet = app
                .get_webview_window(PET_WINDOW)
                .ok_or_else(|| tauri::Error::AssetNotFound(PET_WINDOW.into()))?;
            let area = selected_monitor(&pet, settings.monitor_id.as_deref())?;
            let placement = planner_placement(&settings, area);
            settings.monitor_id = Some(placement.area.id.clone());
            pet.set_size(PhysicalSize::new(
                placement.pet_size_physical,
                placement.pet_size_physical,
            ))?;
            let planner = BehaviorPlanner::new(
                PlannerConfig::default(),
                Lcg::new(current_seed()),
                placement.anchor,
                placement.footing,
                current_ms(),
            );
            let tutorial_bubble = tutorial_directive(settings.tutorial_step, 1, 1);
            store.save(&settings)?;
            app.manage(CoreState {
                store,
                runtime: Mutex::new(RuntimeData {
                    settings,
                    manually_hidden: false,
                    visibility_reason: None,
                    environment_policy: EnvironmentPolicy::default(),
                    drag: None,
                    planner,
                    tutorial_bubble,
                    next_tutorial_bubble_id: 2,
                    next_tutorial_bubble_sequence: 2,
                }),
            });
            {
                let state = app.state::<CoreState>();
                let mut data = lock_runtime(&state);
                apply_m1_position(app.handle(), &data.planner.position_at(current_ms()));
                apply_window_visibility(app.handle(), &data);
                emit_m1_snapshot(app.handle(), &mut data, current_ms());
                emit_tutorial_bubble(app.handle(), &data);
            }

            let handles = create_menu(app)?;
            let tray_menu = handles.menu.clone();
            app.manage(handles);
            let mut tray = TrayIconBuilder::with_id("yunweishou")
                .menu(&tray_menu)
                .show_menu_on_left_click(true)
                .tooltip("云尾兽");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            start_runtime(app.handle().clone());
            Ok(())
        })
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_runtime_snapshot,
            get_tutorial_bubble_directive,
            update_settings,
            set_pet_visible,
            set_m2_environment_policy,
            set_pet_scale,
            set_sound_enabled,
            set_autostart_enabled,
            reset_pet_position,
            input_observed,
            animation_observed,
            show_context_menu,
            quit_app,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_position_round_trips() {
        let area = MonitorArea {
            id: "test".into(),
            x: -1920,
            y: 0,
            width: 1920,
            height: 1040,
            scale_factor: 1.0,
        };
        let x = x_from_normalized(&area, 180, 0.64);
        let normalized = normalized_from_x(&area, 180, x);
        assert!((normalized - 0.64).abs() < 0.001);
    }

    #[test]
    fn position_is_clamped_when_pet_is_wider_than_work_area() {
        let area = MonitorArea {
            id: "tiny".into(),
            x: 20,
            y: 30,
            width: 100,
            height: 100,
            scale_factor: 1.0,
        };
        assert_eq!(x_from_normalized(&area, 260, 0.8), 20);
        assert_eq!(normalized_from_x(&area, 260, 20), 0.0);
    }

    #[test]
    fn settings_reanchor_builds_a_planner_owned_desktop_footing() {
        let settings = PetSettings::default();
        let placement = planner_placement(
            &settings,
            MonitorArea {
                id: "primary".into(),
                x: -1920,
                y: 40,
                width: 1920,
                height: 1040,
                scale_factor: 1.5,
            },
        );
        assert_eq!(placement.anchor.monitor_id, "primary");
        assert_eq!(placement.anchor.y_logical, placement.footing.top_y_logical);
        assert!(placement.footing.contains(&placement.anchor));
    }

    #[test]
    fn manual_hide_has_priority_over_fullscreen_restore() {
        let footing = Footing {
            id: "desktop".into(),
            monitor_id: "primary".into(),
            top_y_logical: 420.0,
            min_x_logical: 0.0,
            max_x_logical: 500.0,
            source: FootingSource::DesktopWorkArea,
        };
        let data = RuntimeData {
            settings: PetSettings::default(),
            manually_hidden: true,
            visibility_reason: None,
            environment_policy: EnvironmentPolicy::default(),
            drag: None,
            planner: BehaviorPlanner::new(
                PlannerConfig::default(),
                Lcg::new(1),
                WorldPoint::new("primary", 100.0, 420.0),
                footing,
                0,
            ),
            tutorial_bubble: tutorial_directive(0, 1, 1),
            next_tutorial_bubble_id: 2,
            next_tutorial_bubble_sequence: 2,
        };
        assert!(!effective_visible(&data));
    }
}
