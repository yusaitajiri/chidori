//! What Iced leaves to the platform: the borderless window's shadow and corners, keeping it out
//! of screen captures, and one running copy of the app (FR-91).

use crate::bridge::{self, UiEvent};

/// Gives the borderless window the system's shadow and, on Windows 11, its rounded corners.
pub fn apply_chrome(hwnd: u64) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
            DwmSetWindowAttribute,
        };
        use windows::Win32::UI::Controls::MARGINS;
        let hwnd = HWND(hwnd as isize as *mut _);
        let margins = MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: 1, cyBottomHeight: 1 };
        let corner: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
        // SAFETY: `hwnd` is the app's own live window; the attribute is a plain value read by size.
        unsafe {
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const corner).cast(),
                size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Keeps the window out of screen sharing and screenshots (設定 › スクショ, on by default).
pub fn set_capture_excluded(hwnd: u64, excluded: bool) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE};
        let affinity = if excluded { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
        // SAFETY: `hwnd` is the app's own live window.
        if let Err(e) = unsafe { SetWindowDisplayAffinity(HWND(hwnd as isize as *mut _), affinity) } {
            tracing::warn!("capture exclusion: {e}");
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd, excluded);
}

/// Makes this the one running copy. A second launch signals the first, which shows its window,
/// and returns `false` so the caller exits.
pub fn single_instance() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, WAIT_OBJECT_0};
        use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject};
        use windows::core::w;
        // SAFETY: plain Win32 calls on named objects this process creates or opens.
        unsafe {
            let event = match CreateEventW(None, false, false, w!("Local\\dev.yusai.chidori.show")) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("single instance event: {e}");
                    return true;
                }
            };
            let mutex = CreateMutexW(None, true, w!("Local\\dev.yusai.chidori"));
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = SetEvent(event);
                let _ = CloseHandle(event);
                return false;
            }
            // The mutex lives as long as the process.
            std::mem::forget(mutex);
            let event = event.0 as usize;
            std::thread::Builder::new()
                .name("second-launch".into())
                .spawn(move || {
                    let event = windows::Win32::Foundation::HANDLE(event as *mut _);
                    while WaitForSingleObject(event, INFINITE) == WAIT_OBJECT_0 {
                        bridge::send(UiEvent::SecondInstance);
                    }
                })
                .ok();
        }
        true
    }
    #[cfg(not(windows))]
    true
}
