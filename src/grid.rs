use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[derive(Clone, Copy)]
pub struct Grid {
    pub rect: RECT,
    pub depth: u8,
}

pub fn virtual_screen_rect() -> RECT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }
}

pub fn cell_rect(rect: RECT, number: u8) -> RECT {
    let index = number.saturating_sub(1) as i32;
    let col = index % 3;
    let row = index / 3;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    RECT {
        left: rect.left + width * col / 3,
        top: rect.top + height * row / 3,
        right: rect.left + width * (col + 1) / 3,
        bottom: rect.top + height * (row + 1) / 3,
    }
}

pub fn to_client_rect(rect: RECT, screen: RECT) -> RECT {
    RECT {
        left: rect.left - screen.left,
        top: rect.top - screen.top,
        right: rect.right - screen.left,
        bottom: rect.bottom - screen.top,
    }
}
