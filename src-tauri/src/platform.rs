#[cfg(target_os = "windows")]
mod windows {
    use std::{ffi::c_void, mem::size_of};

    type Hwnd = *mut c_void;
    type Hmonitor = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn IsWindowVisible(window: Hwnd) -> i32;
        fn MonitorFromWindow(window: Hwnd, flags: u32) -> Hmonitor;
        fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfo) -> i32;
    }

    pub fn foreground_is_fullscreen(ignored_windows: &[isize]) -> bool {
        const MONITOR_DEFAULT_TO_NEAREST: u32 = 2;

        // SAFETY: all pointers come from user32, output structs are initialized,
        // and return values are checked before their contents are used.
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.is_null()
                || IsWindowVisible(foreground) == 0
                || ignored_windows.contains(&(foreground as isize))
            {
                return false;
            }

            let mut window_rect = Rect::default();
            if GetWindowRect(foreground, &mut window_rect) == 0 {
                return false;
            }

            let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULT_TO_NEAREST);
            if monitor.is_null() {
                return false;
            }
            let mut info = MonitorInfo {
                cb_size: size_of::<MonitorInfo>() as u32,
                monitor: Rect::default(),
                work: Rect::default(),
                flags: 0,
            };
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return false;
            }

            let tolerance = 2;
            window_rect.left <= info.monitor.left + tolerance
                && window_rect.top <= info.monitor.top + tolerance
                && window_rect.right >= info.monitor.right - tolerance
                && window_rect.bottom >= info.monitor.bottom - tolerance
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::foreground_is_fullscreen;

#[cfg(not(target_os = "windows"))]
pub fn foreground_is_fullscreen(_ignored_windows: &[isize]) -> bool {
    false
}
