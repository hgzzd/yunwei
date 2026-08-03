mod behavior;
mod model;
mod platform;
mod settings;

use behavior::BehaviorEngine;
use model::{
    BubblePayload, Facing, PetScale, PetSettings, PetState, SettingsPatch, StatePayload,
    VisibilityPayload,
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
    behavior: BehaviorEngine,
    manually_hidden: bool,
    fullscreen_hidden: bool,
    drag: Option<DragState>,
    last_state: Option<StatePayload>,
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
    !data.manually_hidden && !data.fullscreen_hidden
}

fn visibility_payload(data: &RuntimeData) -> VisibilityPayload {
    VisibilityPayload {
        visible: effective_visible(data),
        manually_hidden: data.manually_hidden,
        fullscreen_hidden: data.fullscreen_hidden,
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
        } else if data.settings.tutorial_step < 3 {
            let _ = bubble.show();
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

fn position_from_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &mut PetSettings,
) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let area = selected_monitor(&pet, settings.monitor_id.as_deref())?;
    let size = physical_pet_size(settings.scale, &area);
    let x = x_from_normalized(&area, size, settings.normalized_x);
    let y = area.y + area.height as i32 - size as i32;
    pet.set_size(PhysicalSize::new(size, size))
        .map_err(|error| error.to_string())?;
    pet.set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    settings.monitor_id = Some(area.id.clone());
    place_bubble(app, x, y, &area);
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
    position_from_settings(&app, &mut data.settings)?;
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
    sync_menu(&app, &data);
    Ok(visibility_payload(&data))
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
    position_from_settings(&app, &mut data.settings)?;
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
) -> Result<StatePayload, String> {
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
    data.behavior.begin_drag();
    let payload = data.behavior.payload();
    let _ = app.emit("pet://state", payload.clone());
    Ok(payload)
}

#[tauri::command]
fn drag_pet(
    app: AppHandle,
    state: State<'_, CoreState>,
    pointer_x: f64,
    pointer_y: f64,
) -> Result<(), String> {
    let drag = lock_runtime(&state)
        .drag
        .ok_or_else(|| "尚未开始拖动".to_string())?;
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let cursor = pet
        .cursor_position()
        .unwrap_or_else(|_| PhysicalPosition::new(pointer_x, pointer_y));
    let x = (cursor.x - drag.offset_x).round() as i32;
    let y = (cursor.y - drag.offset_y).round() as i32;
    pet.set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    if let (Ok(size), Ok(areas)) = (pet.outer_size(), monitor_areas(&pet)) {
        let center_x = f64::from(x) + f64::from(size.width) / 2.0;
        let center_y = f64::from(y) + f64::from(size.height) / 2.0;
        if let Some(area) = areas.iter().find(|area| area.contains(center_x, center_y)) {
            place_bubble(&app, x, y, area);
        }
    }
    Ok(())
}

#[tauri::command]
fn end_drag(app: AppHandle, state: State<'_, CoreState>) -> Result<PetSettings, String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    let drag = lock_runtime(&state).drag;
    let position = if let (Some(drag), Ok(cursor)) = (drag, pet.cursor_position()) {
        let position = PhysicalPosition::new(
            (cursor.x - drag.offset_x).round() as i32,
            (cursor.y - drag.offset_y).round() as i32,
        );
        let _ = pet.set_position(position);
        position
    } else {
        pet.outer_position().map_err(|error| error.to_string())?
    };
    let size = pet.outer_size().map_err(|error| error.to_string())?;
    let center_x = f64::from(position.x) + f64::from(size.width) / 2.0;
    let center_y = f64::from(position.y) + f64::from(size.height) / 2.0;
    let areas = monitor_areas(&pet)?;
    let area = areas
        .iter()
        .find(|area| area.contains(center_x, center_y))
        .cloned()
        .or_else(|| {
            areas.iter().cloned().min_by_key(|area| {
                let dx = center_x - (f64::from(area.x) + f64::from(area.width) / 2.0);
                let dy = center_y - (f64::from(area.y) + f64::from(area.height) / 2.0);
                (dx * dx + dy * dy) as u64
            })
        })
        .ok_or_else(|| "没有检测到可用显示器".to_string())?;

    let mut data = lock_runtime(&state);
    let pet_size = physical_pet_size(data.settings.scale, &area);
    let clamped_x = position
        .x
        .clamp(area.x, area.x + area.width.saturating_sub(pet_size) as i32);
    let y = area.y + area.height as i32 - pet_size as i32;
    pet.set_size(PhysicalSize::new(pet_size, pet_size))
        .map_err(|error| error.to_string())?;
    pet.set_position(PhysicalPosition::new(clamped_x, y))
        .map_err(|error| error.to_string())?;
    place_bubble(&app, clamped_x, y, &area);
    data.settings.normalized_x = normalized_from_x(&area, pet_size, clamped_x);
    data.settings.monitor_id = Some(area.id);
    data.drag = None;
    data.behavior.end_drag();
    save_settings(&state, &data.settings)?;
    emit_settings(&app, &data.settings);
    Ok(data.settings.clone())
}

#[tauri::command]
fn pet_clicked(app: AppHandle, state: State<'_, CoreState>) -> Result<StatePayload, String> {
    let mut data = lock_runtime(&state);
    let bubble = data.behavior.clicked();
    if data.settings.tutorial_step >= 3 {
        show_bubble(&app, &bubble);
    }
    let payload = data.behavior.payload();
    let _ = app.emit("pet://state", payload.clone());
    Ok(payload)
}

#[tauri::command]
fn tutorial_advanced(
    app: AppHandle,
    state: State<'_, CoreState>,
    step: u8,
) -> Result<PetSettings, String> {
    let mut data = lock_runtime(&state);
    data.settings.tutorial_step = step.min(3);
    save_settings(&state, &data.settings)?;
    emit_settings(&app, &data.settings);
    if let Some(bubble) = app.get_webview_window(BUBBLE_WINDOW) {
        if effective_visible(&data) && data.settings.tutorial_step < 3 {
            let _ = bubble.show();
        } else {
            let _ = bubble.hide();
        }
    }
    Ok(data.settings.clone())
}

#[tauri::command]
fn show_context_menu(app: AppHandle, menus: State<'_, MenuHandles>) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_WINDOW)
        .ok_or_else(|| "pet 窗口不存在".to_string())?;
    pet.popup_menu(&menus.menu)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn show_bubble<R: Runtime>(app: &AppHandle<R>, payload: &BubblePayload) {
    let _ = app.emit("pet://bubble", payload.clone());
    let Some(window) = app.get_webview_window(BUBBLE_WINDOW) else {
        return;
    };
    if payload.visible {
        if let Some(pet) = app.get_webview_window(PET_WINDOW) {
            if let (Ok(position), Ok(size), Ok(areas)) =
                (pet.outer_position(), pet.outer_size(), monitor_areas(&pet))
            {
                let center_x = f64::from(position.x) + f64::from(size.width) / 2.0;
                let center_y = f64::from(position.y) + f64::from(size.height) / 2.0;
                if let Some(area) = areas.iter().find(|area| area.contains(center_x, center_y)) {
                    place_bubble(app, position.x, position.y, area);
                }
            }
        }
        let _ = window.show();
    } else {
        let _ = window.hide();
    }
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
        let mut fullscreen_check_due = 0_u8;
        loop {
            let state = app.state::<CoreState>();
            let visible = {
                let data = lock_runtime(&state);
                effective_visible(&data)
            };

            let sleep_ms = if visible { 33 } else { 250 };
            thread::sleep(Duration::from_millis(sleep_ms));

            fullscreen_check_due = fullscreen_check_due.saturating_add(1);
            let check_every = if visible { 30 } else { 4 };
            if fullscreen_check_due >= check_every {
                fullscreen_check_due = 0;
                update_fullscreen_visibility(&app);
                ensure_pet_geometry(&app);
            }

            let mut data = lock_runtime(&state);
            if !effective_visible(&data) {
                continue;
            }
            let result = data.behavior.tick(sleep_ms);
            if data.last_state.as_ref() != Some(&result.state) {
                data.last_state = Some(result.state.clone());
                let _ = app.emit("pet://state", result.state.clone());
            }
            if data.drag.is_none() && data.behavior.state().moves() {
                move_pet(&app, &mut data, sleep_ms);
            }
            if let Some(bubble) = result.bubble {
                if data.settings.tutorial_step >= 3 {
                    show_bubble(&app, &bubble);
                }
            }
        }
    });
}

fn ensure_pet_geometry(app: &AppHandle) {
    let Some(pet) = app.get_webview_window(PET_WINDOW) else {
        return;
    };
    let state = app.state::<CoreState>();
    let mut data = lock_runtime(&state);
    if data.drag.is_some() {
        return;
    }
    let Ok(areas) = monitor_areas(&pet) else {
        return;
    };
    if areas.is_empty() {
        return;
    }

    let requested_monitor_exists = data
        .settings
        .monitor_id
        .as_ref()
        .is_some_and(|id| areas.iter().any(|area| &area.id == id));
    if !requested_monitor_exists {
        data.settings.monitor_id = None;
        data.settings.normalized_x = PetSettings::default().normalized_x;
        if position_from_settings(app, &mut data.settings).is_ok() {
            let _ = save_settings(&state, &data.settings);
            emit_settings(app, &data.settings);
        }
        return;
    }

    let (Ok(position), Ok(size)) = (pet.outer_position(), pet.outer_size()) else {
        return;
    };
    let Some(area) = areas.iter().find(|area| {
        data.settings
            .monitor_id
            .as_ref()
            .is_some_and(|id| &area.id == id)
    }) else {
        return;
    };
    let expected_size = physical_pet_size(data.settings.scale, area);
    let expected_y = area.y + area.height as i32 - expected_size as i32;
    if size.width != expected_size || size.height != expected_size {
        data.settings.normalized_x = normalized_from_x(area, size.width, position.x);
        let x = x_from_normalized(area, expected_size, data.settings.normalized_x);
        let _ = pet.set_size(PhysicalSize::new(expected_size, expected_size));
        let _ = pet.set_position(PhysicalPosition::new(x, expected_y));
        place_bubble(app, x, expected_y, area);
        let _ = save_settings(&state, &data.settings);
        emit_settings(app, &data.settings);
    } else if position.y != expected_y {
        let x = position.x.clamp(
            area.x,
            area.x + area.width.saturating_sub(expected_size) as i32,
        );
        let _ = pet.set_position(PhysicalPosition::new(x, expected_y));
        place_bubble(app, x, expected_y, area);
    }
}

fn move_pet(app: &AppHandle, data: &mut RuntimeData, elapsed_ms: u64) {
    let Some(pet) = app.get_webview_window(PET_WINDOW) else {
        return;
    };
    let (Ok(position), Ok(size), Ok(areas)) =
        (pet.outer_position(), pet.outer_size(), monitor_areas(&pet))
    else {
        return;
    };
    let center_x = f64::from(position.x) + f64::from(size.width) / 2.0;
    let center_y = f64::from(position.y) + f64::from(size.height) / 2.0;
    let Some(area) = areas.iter().find(|area| area.contains(center_x, center_y)) else {
        return;
    };
    let logical_speed = if data.behavior.state() == PetState::Running {
        150.0
    } else {
        65.0
    };
    let delta = (logical_speed * area.scale_factor * elapsed_ms as f64 / 1_000.0).max(1.0);
    let direction = if data.behavior.facing() == Facing::Right {
        1.0
    } else {
        -1.0
    };
    let min_x = area.x;
    let max_x = area.x + area.width.saturating_sub(size.width) as i32;
    let proposed = (f64::from(position.x) + direction * delta).round() as i32;
    let x = proposed.clamp(min_x, max_x.max(min_x));
    if proposed != x {
        data.behavior.reverse();
    }
    let y = area.y + area.height as i32 - size.height as i32;
    let _ = pet.set_position(PhysicalPosition::new(x, y));
    place_bubble(app, x, y, area);
}

fn update_fullscreen_visibility(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    let ignored = [PET_WINDOW, BUBBLE_WINDOW]
        .iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter_map(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
        .collect::<Vec<_>>();
    #[cfg(not(target_os = "windows"))]
    let ignored = Vec::new();

    let fullscreen = platform::foreground_is_fullscreen(&ignored);
    let state = app.state::<CoreState>();
    let mut data = lock_runtime(&state);
    if data.fullscreen_hidden != fullscreen {
        data.fullscreen_hidden = fullscreen;
        apply_window_visibility(app, &data);
    }
}

fn current_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1)
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
            store.save(&settings)?;
            app.manage(CoreState {
                store,
                runtime: Mutex::new(RuntimeData {
                    settings,
                    behavior: BehaviorEngine::new(current_seed()),
                    manually_hidden: false,
                    fullscreen_hidden: false,
                    drag: None,
                    last_state: None,
                }),
            });

            configure_windows(app)?;
            {
                let state = app.state::<CoreState>();
                let mut data = lock_runtime(&state);
                position_from_settings(app.handle(), &mut data.settings)?;
                state.store.save(&data.settings)?;
                apply_window_visibility(app.handle(), &data);
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
            update_settings,
            set_pet_visible,
            set_pet_scale,
            set_sound_enabled,
            set_autostart_enabled,
            reset_pet_position,
            begin_drag,
            drag_pet,
            end_drag,
            pet_clicked,
            tutorial_advanced,
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
    fn manual_hide_has_priority_over_fullscreen_restore() {
        let data = RuntimeData {
            settings: PetSettings::default(),
            behavior: BehaviorEngine::new(1),
            manually_hidden: true,
            fullscreen_hidden: false,
            drag: None,
            last_state: None,
        };
        assert!(!effective_visible(&data));
    }
}
