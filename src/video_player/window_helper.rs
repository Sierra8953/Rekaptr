use anyhow::Result;
use std::sync::Once;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOW,
    WINDOW_EX_STYLE, WNDCLASSW, WS_CHILD, WS_VISIBLE,
};

static REGISTER_CLASS: Once = Once::new();

#[derive(Clone, Copy)]
pub struct VideoWindow {
    pub hwnd: HWND,
}

impl VideoWindow {
    pub fn new(parent: HWND, x: i32, y: i32, width: i32, height: i32) -> Result<Self> {
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        unsafe {
            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
            let class_name = w!("LumaVideoClass");

            REGISTER_CLASS.call_once(|| {
                // Use a black brush for the background
                let brush = CreateSolidBrush(COLORREF(0));
                let wnd_class = WNDCLASSW {
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: HINSTANCE(instance.0),
                    lpszClassName: class_name,
                    hbrBackground: brush,
                    ..Default::default()
                };
                RegisterClassW(&wnd_class);
            });

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("Video Preview"),
                WS_CHILD | WS_VISIBLE,
                x,
                y,
                width,
                height,
                Some(parent),
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )?;

            // Ensure the window is at the top of the Z-order
            let _ = SetWindowPos(
                hwnd,
                Some(HWND(std::ptr::null_mut())), // HWND_TOP
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );

            let _ = ShowWindow(hwnd, SW_SHOW);

            Ok(Self { hwnd })
        }
    }

    pub fn set_geometry(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub struct VideoWindowHandle(pub VideoWindow);
impl Drop for VideoWindowHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0.hwnd);
        }
    }
}
