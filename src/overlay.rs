//! 81 宫格透明覆盖窗口和 GDI 绘制。
//!
//! 覆盖窗口始终位于普通窗口上方，但不获取焦点且允许鼠标点击穿透。
//! 黑色被配置为透明色键，红色线条和数字则作为可见的 81 宫格提示。

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

use crate::grid::{Grid, cell_rect, row_rect, to_client_rect, virtual_screen_rect};
use crate::state;

/// 全局保存覆盖窗口句柄。
///
/// Windows 句柄本质上是指针大小的值，因此使用 `AtomicIsize` 跨回调安全读取。
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);

/// 创建并保存覆盖窗口。
///
/// 成功返回 `true`；失败返回 `false`，此时 81 宫格功能无法工作。
pub fn init(instance: HINSTANCE) -> bool {
    let hwnd = unsafe { create_overlay_window(instance) };
    if hwnd.is_null() {
        return false;
    }

    OVERLAY_HWND.store(hwnd as isize, Ordering::Relaxed);
    true
}

/// 显示覆盖窗口并触发完整重绘。
pub fn show() {
    let hwnd = hwnd();
    if hwnd.is_null() {
        return;
    }

    // 每次显示都重新获取虚拟桌面尺寸，以适应运行期间的显示器布局变化。
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
        // SW_SHOWNOACTIVATE 保证显示 81 宫格时不会抢走前台程序焦点。
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        InvalidateRect(hwnd, ptr::null::<RECT>(), 1 as BOOL);
    }
}

/// 隐藏覆盖窗口。
pub fn hide() {
    let hwnd = hwnd();
    if hwnd.is_null() {
        return;
    }

    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// 覆盖窗口的 Windows 消息处理函数。
unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 只自行处理 WM_PAINT，其余消息交给默认窗口过程。
    if msg == WM_PAINT {
        paint(hwnd);
        return 0;
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 响应 WM_PAINT，清空旧画面并绘制当前 81 宫格。
fn paint(hwnd: HWND) {
    unsafe {
        // BeginPaint/EndPaint 必须成对调用，Windows 才会清除窗口的无效区域标记。
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

        // 黑色是透明色键；先填满客户区可清除上一帧残留红线。
        let black = CreateSolidBrush(0x00000000);
        FillRect(dc, &client, black);
        DeleteObject(black as HGDIOBJ);

        // Grid 保存虚拟桌面坐标，绘制前转换为覆盖窗口客户区坐标。
        if let Some(app) = state::lock()
            && let Some(grid) = app.grid
        {
            draw_grid(
                dc,
                Grid {
                    rect: to_client_rect(grid.rect, screen),
                    ..grid
                },
            );
        }

        EndPaint(hwnd, &paint);
    }
}

/// 注册窗口类并创建隐藏的透明置顶窗口。
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
        // LAYERED 支持颜色键透明；TRANSPARENT 允许点击穿透；
        // TOOLWINDOW 避免出现在 Alt+Tab；TOPMOST 保证 81 宫格可见。
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
            // 将黑色设为完全透明，红色 81 宫格仍保持可见。
            SetLayeredWindowAttributes(hwnd, 0x00000000, 255, LWA_COLORKEY);
            ShowWindow(hwnd, SW_HIDE);
        }

        hwnd
    }
}

/// 根据输入阶段绘制完整 81 宫格、选中行或最终单元格。
fn draw_grid(dc: HDC, grid: Grid) {
    unsafe {
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH) as HBRUSH as HGDIOBJ);

        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x000000ff);

        match (grid.row, grid.col) {
            (None, _) => {
                draw_grid_lines(dc, grid.rect);
                for row in 1..=9 {
                    draw_centered_text(dc, cell_rect(grid.rect, row, 1), &row.to_string());
                }
            }
            (Some(row), None) => {
                draw_grid_lines(dc, grid.rect);
                draw_highlight_rect(dc, row_rect(grid.rect, row));
                for col in 1..=9 {
                    draw_centered_text(dc, cell_rect(grid.rect, row, col), &col.to_string());
                }
            }
            (Some(row), Some(col)) => {
                let cell = cell_rect(grid.rect, row, col);
                draw_highlight_rect(dc, cell);
                draw_text(dc, cell.left + 12, cell.top + 12, "Enter");
            }
        }

        SelectObject(dc, old_brush);
    }
}

/// 绘制 9 行 9 列的完整 81 宫格。
fn draw_grid_lines(dc: HDC, rect: RECT) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 1, 0x000000ff);
        let old_pen = SelectObject(dc, pen as HGDIOBJ);
        let cell_width = (rect.right - rect.left) / 9;
        let cell_height = (rect.bottom - rect.top) / 9;

        Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
        for i in 1..9 {
            let x = rect.left + cell_width * i;
            MoveToEx(dc, x, rect.top, ptr::null_mut::<POINT>());
            LineTo(dc, x, rect.bottom);

            let y = rect.top + cell_height * i;
            MoveToEx(dc, rect.left, y, ptr::null_mut::<POINT>());
            LineTo(dc, rect.right, y);
        }

        SelectObject(dc, old_pen);
        DeleteObject(pen as HGDIOBJ);
    }
}

/// 使用较粗画笔突出当前选择的行或最终单元格。
fn draw_highlight_rect(dc: HDC, rect: RECT) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 4, 0x000000ff);
        let old_pen = SelectObject(dc, pen as HGDIOBJ);
        Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
        SelectObject(dc, old_pen);
        DeleteObject(pen as HGDIOBJ);
    }
}

/// 使用系统默认 GDI 字体，将文本近似绘制在矩形中心。
fn draw_centered_text(dc: HDC, rect: RECT, text: &str) {
    let char_width = 8;
    let char_height = 16;
    let text_width = text.encode_utf16().count() as i32 * char_width;
    let x = rect.left + ((rect.right - rect.left - text_width) / 2).max(0);
    let y = rect.top + ((rect.bottom - rect.top - char_height) / 2).max(0);
    draw_text(dc, x, y, text);
}

/// 将 UTF-8 Rust 字符串转换为 UTF-16 并绘制到指定位置。
fn draw_text(dc: HDC, x: i32, y: i32, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        TextOutW(dc, x, y, wide.as_ptr(), wide.len() as i32);
    }
}

/// 读取全局覆盖窗口句柄。
fn hwnd() -> HWND {
    OVERLAY_HWND.load(Ordering::Relaxed) as HWND
}

/// 将 Rust 字符串转换为以 NUL 结尾的 UTF-16 Windows 字符串。
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
