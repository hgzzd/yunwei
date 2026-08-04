//! Thin, platform-specific environment capture.
//!
//! This module deliberately only reads native state and converts it into plain
//! Rust values.  Monitor selection, visibility priority, fullscreen policy,
//! and footing decisions belong in `environment`, where they can be tested
//! without Windows.

use crate::environment::{
    EnvironmentPort, EnvironmentSnapshot, ForegroundKind, ForegroundWindowSnapshot,
    MonitorSnapshot, PhysicalRect,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlatformRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformMonitor {
    /// Windows display device name (for example `\\\\.\\DISPLAY1`).
    pub id: String,
    /// Full physical monitor bounds in virtual-desktop coordinates.
    pub monitor_rect: PlatformRect,
    /// Physical work-area bounds in virtual-desktop coordinates.
    pub work_rect: PlatformRect,
    pub dpi_x: u32,
    pub dpi_y: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformForegroundWindow {
    pub handle: isize,
    pub rect: PlatformRect,
    pub monitor_id: Option<String>,
    pub process_id: u32,
    pub class_name: String,
    pub title: String,
    pub app_user_model_id: Option<String>,
    pub executable_path: Option<String>,
    pub is_visible: bool,
    pub is_minimized: bool,
    pub is_desktop_shell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformEnvironment {
    pub captured_at_ms: u64,
    pub monitors: Vec<PlatformMonitor>,
    pub foreground: Option<PlatformForegroundWindow>,
}

impl PlatformEnvironment {
    fn empty(captured_at_ms: u64) -> Self {
        Self {
            captured_at_ms,
            monitors: Vec::new(),
            foreground: None,
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{PlatformEnvironment, PlatformForegroundWindow, PlatformMonitor, PlatformRect};
    use std::{ffi::c_void, mem::size_of, ptr};

    type Hwnd = *mut c_void;
    type Hmonitor = *mut c_void;
    type Hdc = *mut c_void;
    type Handle = *mut c_void;
    type Lparam = isize;
    type MonitorEnumProc =
        Option<unsafe extern "system" fn(Hmonitor, Hdc, *mut Rect, Lparam) -> i32>;

    const MONITOR_DEFAULT_TO_NEAREST: u32 = 2;
    const MONITORINFOF_PRIMARY: u32 = 1;
    const MDT_EFFECTIVE_DPI: i32 = 0;
    const DEFAULT_DPI: u32 = 96;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const ERROR_SUCCESS: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    impl From<Rect> for PlatformRect {
        fn from(value: Rect) -> Self {
            Self {
                left: value.left,
                top: value.top,
                right: value.right,
                bottom: value.bottom,
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct MonitorInfoExW {
        cb_size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
        device: [u16; 32],
    }

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: Hdc,
            clip: *const Rect,
            callback: MonitorEnumProc,
            data: Lparam,
        ) -> i32;
        fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfoExW) -> i32;
        fn MonitorFromWindow(window: Hwnd, flags: u32) -> Hmonitor;
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn IsWindowVisible(window: Hwnd) -> i32;
        fn IsIconic(window: Hwnd) -> i32;
        fn GetClassNameW(window: Hwnd, buffer: *mut u16, max_count: i32) -> i32;
        fn GetWindowTextW(window: Hwnd, buffer: *mut u16, max_count: i32) -> i32;
        fn GetShellWindow() -> Hwnd;
        fn GetDesktopWindow() -> Hwnd;
        fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
    }

    #[link(name = "shcore")]
    extern "system" {
        fn GetDpiForMonitor(
            monitor: Hmonitor,
            dpi_type: i32,
            dpi_x: *mut u32,
            dpi_y: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            buffer: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn GetApplicationUserModelId(process: Handle, length: *mut u32, buffer: *mut u16) -> u32;
    }

    fn utf16_string(buffer: &[u16], length: usize) -> String {
        String::from_utf16_lossy(&buffer[..length.min(buffer.len())])
            .trim_end_matches('\0')
            .to_owned()
    }

    unsafe fn monitor_info(monitor: Hmonitor) -> Option<MonitorInfoExW> {
        let mut info = MonitorInfoExW {
            cb_size: size_of::<MonitorInfoExW>() as u32,
            ..Default::default()
        };
        (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info)
    }

    unsafe fn monitor_dpi(monitor: Hmonitor) -> (u32, u32) {
        let mut dpi_x = DEFAULT_DPI;
        let mut dpi_y = DEFAULT_DPI;
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) < 0 {
            (DEFAULT_DPI, DEFAULT_DPI)
        } else {
            (dpi_x.max(1), dpi_y.max(1))
        }
    }

    unsafe extern "system" fn collect_monitor(
        monitor: Hmonitor,
        _hdc: Hdc,
        _clip: *mut Rect,
        data: Lparam,
    ) -> i32 {
        // SAFETY: `data` is a pointer to the Vec supplied by capture_environment,
        // and EnumDisplayMonitors invokes the callback synchronously.
        let monitors = unsafe { &mut *(data as *mut Vec<PlatformMonitor>) };
        if let Some(info) = unsafe { monitor_info(monitor) } {
            let (dpi_x, dpi_y) = unsafe { monitor_dpi(monitor) };
            monitors.push(PlatformMonitor {
                id: utf16_string(&info.device, info.device.len()),
                monitor_rect: info.monitor.into(),
                work_rect: info.work.into(),
                dpi_x,
                dpi_y,
                is_primary: info.flags & MONITORINFOF_PRIMARY != 0,
            });
        }
        1
    }

    fn window_string(
        window: Hwnd,
        getter: unsafe extern "system" fn(Hwnd, *mut u16, i32) -> i32,
    ) -> String {
        let mut buffer = [0_u16; 512];
        // SAFETY: the buffer is valid for the supplied capacity and the hwnd came
        // from user32. A failed call is represented by an empty string.
        let length = unsafe { getter(window, buffer.as_mut_ptr(), buffer.len() as i32) };
        if length <= 0 {
            String::new()
        } else {
            utf16_string(&buffer, length as usize)
        }
    }

    unsafe fn process_identity(process_id: u32) -> (Option<String>, Option<String>) {
        if process_id == 0 {
            return (None, None);
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return (None, None);
        }

        let mut image = vec![0_u16; 32_768];
        let mut image_len = image.len() as u32;
        let executable_path =
            (QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut image_len) != 0)
                .then(|| utf16_string(&image, image_len as usize))
                .filter(|path| !path.is_empty());

        let mut app_id_len = 0_u32;
        let status = GetApplicationUserModelId(process, &mut app_id_len, ptr::null_mut());
        let app_user_model_id = if status == ERROR_INSUFFICIENT_BUFFER && app_id_len > 0 {
            let mut app_id = vec![0_u16; app_id_len as usize];
            (GetApplicationUserModelId(process, &mut app_id_len, app_id.as_mut_ptr())
                == ERROR_SUCCESS)
                .then(|| utf16_string(&app_id, app_id_len as usize))
                .filter(|id| !id.is_empty())
        } else {
            None
        };

        let _ = CloseHandle(process);
        (app_user_model_id, executable_path)
    }

    unsafe fn foreground_snapshot(ignored_windows: &[isize]) -> Option<PlatformForegroundWindow> {
        let window = GetForegroundWindow();
        if window.is_null() || ignored_windows.contains(&(window as isize)) {
            return None;
        }

        let is_visible = IsWindowVisible(window) != 0;
        let is_minimized = IsIconic(window) != 0;
        let mut rect = Rect::default();
        if GetWindowRect(window, &mut rect) == 0 {
            return None;
        }
        let class_name = window_string(window, GetClassNameW);
        let title = window_string(window, GetWindowTextW);
        let desktop = window == GetDesktopWindow()
            || window == GetShellWindow()
            || matches!(class_name.as_str(), "Progman" | "WorkerW");

        let mut process_id = 0_u32;
        let _ = GetWindowThreadProcessId(window, &mut process_id);
        let (app_user_model_id, executable_path) = process_identity(process_id);
        let monitor_id = monitor_info(MonitorFromWindow(window, MONITOR_DEFAULT_TO_NEAREST))
            .map(|info| utf16_string(&info.device, info.device.len()))
            .filter(|id| !id.is_empty());

        Some(PlatformForegroundWindow {
            handle: window as isize,
            rect: rect.into(),
            monitor_id,
            process_id,
            class_name,
            title,
            app_user_model_id,
            executable_path,
            is_visible,
            is_minimized,
            is_desktop_shell: desktop,
        })
    }

    pub fn capture_platform_environment(
        ignored_windows: &[isize],
        captured_at_ms: u64,
    ) -> PlatformEnvironment {
        let mut environment = PlatformEnvironment::empty(captured_at_ms);
        // SAFETY: collection context is valid until the synchronous API call returns.
        unsafe {
            let _ = EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(collect_monitor),
                (&mut environment.monitors as *mut Vec<PlatformMonitor>) as isize,
            );
            environment.foreground = foreground_snapshot(ignored_windows);
        }
        environment
    }
}

#[cfg(target_os = "windows")]
pub use windows::capture_platform_environment;

#[cfg(not(target_os = "windows"))]
pub fn capture_platform_environment(
    _ignored_windows: &[isize],
    captured_at_ms: u64,
) -> PlatformEnvironment {
    PlatformEnvironment::empty(captured_at_ms)
}

fn physical_rect(rect: PlatformRect) -> PhysicalRect {
    PhysicalRect::new(rect.left, rect.top, rect.right, rect.bottom)
}

fn normalized_executable_path(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// Captures native Windows state and converts it to the domain-owned snapshot.
/// No monitor choice, hiding priority, or footing decision is made here.
pub fn capture_environment(ignored_windows: &[isize], captured_at_ms: u64) -> EnvironmentSnapshot {
    let platform = capture_platform_environment(ignored_windows, captured_at_ms);
    let monitors = platform
        .monitors
        .iter()
        .map(|monitor| MonitorSnapshot {
            id: monitor.id.clone(),
            work_area_physical: physical_rect(monitor.work_rect),
            monitor_area_physical: physical_rect(monitor.monitor_rect),
            scale_factor: f64::from(monitor.dpi_x.max(1)) / 96.0,
            is_primary: monitor.is_primary,
        })
        .collect();
    let foreground = platform.foreground.map(|window| ForegroundWindowSnapshot {
        app_id: window.app_user_model_id.clone().or_else(|| {
            window
                .executable_path
                .as_deref()
                .map(normalized_executable_path)
        }),
        title: (!window.title.is_empty()).then_some(window.title.clone()),
        rect_physical: physical_rect(window.rect),
        visible: window.is_visible && !window.is_minimized,
        // Fullscreen is a domain decision based on this rectangle and the
        // monitor snapshot, evaluated by EnvironmentPolicy.
        is_fullscreen: false,
        kind: if window.is_desktop_shell {
            ForegroundKind::DesktopShell
        } else {
            ForegroundKind::Normal
        },
    });
    EnvironmentSnapshot {
        monitors,
        foreground,
        captured_at_ms: platform.captured_at_ms,
    }
}

pub struct WindowsEnvironmentPort {
    ignored_windows: Vec<isize>,
}

impl WindowsEnvironmentPort {
    pub fn new(ignored_windows: Vec<isize>) -> Self {
        Self { ignored_windows }
    }
}

impl EnvironmentPort for WindowsEnvironmentPort {
    fn snapshot(&self, captured_at_ms: u64) -> EnvironmentSnapshot {
        capture_environment(&self.ignored_windows, captured_at_ms)
    }
}
