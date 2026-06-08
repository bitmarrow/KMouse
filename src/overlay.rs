use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
    HBRUSH, HDC, HGDIOBJ, HOLLOW_BRUSH, InvalidateRect, LineTo, MoveToEx, PAINTSTRUCT, PS_SOLID,
    Rectangle, SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, HWND_TOPMOST, LWA_COLORKEY,
    RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_PAINT, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::config::MAX_GRID_DEPTH;
use crate::grid::{cell_rect, to_client_rect, virtual_screen_rect};
use crate::state;

static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);

pub fn init(instance: HINSTANCE) -> bool {
    let hwnd = unsafe { create_overlay_window(instance) };
    if hwnd.is_null() {
        return false;
    }

    OVERLAY_HWND.store(hwnd as isize, Ordering::Relaxed);
    true
}

pub fn show() {
    let hwnd = hwnd();
    if hwnd.is_null() {
        return;
    }

    let screen = virtual_screen_rect();
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            screen.left,
            screen.top,
            screen.right - screen.left,
            screen.bottom - screen.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        InvalidateRect(hwnd, ptr::null::<RECT>(), 1 as BOOL);
    }
}

pub fn hide() {
    let hwnd = hwnd();
    if hwnd.is_null() {
        return;
    }

    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        paint(hwnd);
        return 0;
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn paint(hwnd: HWND) {
    unsafe {
        let mut paint = MaybeUninit::<PAINTSTRUCT>::zeroed();
        let dc = BeginPaint(hwnd, paint.as_mut_ptr());
        let paint = paint.assume_init();

        let screen = virtual_screen_rect();
        let client = RECT {
            left: 0,
            top: 0,
            right: screen.right - screen.left,
            bottom: screen.bottom - screen.top,
        };

        let black = CreateSolidBrush(0x00000000);
        FillRect(dc, &client, black);
        DeleteObject(black as HGDIOBJ);

        if let Some(app) = state::lock()
            && let Some(grid) = app.grid
        {
            let rect = to_client_rect(grid.rect, screen);
            draw_grid(dc, rect, grid.depth);
        }

        EndPaint(hwnd, &paint);
    }
}

unsafe fn create_overlay_window(instance: HINSTANCE) -> HWND {
    let class_name = wide("KMouseOverlay");
    let wnd = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: 0 as _,
        hCursor: 0 as _,
        hbrBackground: 0 as _,
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };

    unsafe {
        RegisterClassW(&wnd);

        let screen = virtual_screen_rect();
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            wide("KMouse Grid").as_ptr(),
            WS_POPUP,
            screen.left,
            screen.top,
            screen.right - screen.left,
            screen.bottom - screen.top,
            0 as HWND,
            0 as _,
            instance,
            ptr::null(),
        );

        if !hwnd.is_null() {
            SetLayeredWindowAttributes(hwnd, 0x00000000, 255, LWA_COLORKEY);
            ShowWindow(hwnd, SW_HIDE);
        }

        hwnd
    }
}

fn draw_grid(dc: HDC, rect: RECT, depth: u8) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 3, 0x000000ff);
        let old_pen = SelectObject(dc, pen as HGDIOBJ);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH) as HBRUSH as HGDIOBJ);

        Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);

        if depth < MAX_GRID_DEPTH {
            draw_grid_lines(dc, rect);
        }

        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x000000ff);

        if depth < MAX_GRID_DEPTH {
            for number in 1..=9 {
                let cell = cell_rect(rect, number);
                draw_centered_text(dc, cell, &number.to_string());
            }
        } else {
            draw_text(dc, rect.left + 12, rect.top + 12, "Enter");
        }

        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen as HGDIOBJ);
    }
}

fn draw_grid_lines(dc: HDC, rect: RECT) {
    unsafe {
        let third_w = (rect.right - rect.left) / 3;
        let third_h = (rect.bottom - rect.top) / 3;

        for i in 1..3 {
            let x = rect.left + third_w * i;
            MoveToEx(dc, x, rect.top, ptr::null_mut::<POINT>());
            LineTo(dc, x, rect.bottom);

            let y = rect.top + third_h * i;
            MoveToEx(dc, rect.left, y, ptr::null_mut::<POINT>());
            LineTo(dc, rect.right, y);
        }
    }
}

fn draw_centered_text(dc: HDC, rect: RECT, text: &str) {
    let char_width = 8;
    let char_height = 16;
    let text_width = text.encode_utf16().count() as i32 * char_width;
    let x = rect.left + ((rect.right - rect.left - text_width) / 2).max(0);
    let y = rect.top + ((rect.bottom - rect.top - char_height) / 2).max(0);
    draw_text(dc, x, y, text);
}

fn draw_text(dc: HDC, x: i32, y: i32, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        TextOutW(dc, x, y, wide.as_ptr(), wide.len() as i32);
    }
}

fn hwnd() -> HWND {
    OVERLAY_HWND.load(Ordering::Relaxed) as HWND
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
