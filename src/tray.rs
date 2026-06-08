//! Windows 系统托盘图标、状态显示和右键菜单。
//!
//! 托盘需要一个隐藏窗口接收 Shell 回调消息。右键菜单可切换鼠标模式或退出程序；
//! 托盘图标和提示文本会随鼠标模式状态变化。

use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, GetCursorPos, HICON, MF_SEPARATOR, MF_STRING, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_RBUTTONUP,
    WNDCLASSW,
};

/// 托盘图标在隐藏窗口中的唯一编号。
const TRAY_ICON_ID: u32 = 1;
/// Shell 将托盘鼠标事件发送到隐藏窗口时使用的自定义消息编号。
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
/// “退出”菜单项命令编号。
const EXIT_COMMAND_ID: usize = 1;
/// “启动/关闭鼠标模式”菜单项命令编号。
const TOGGLE_MODE_COMMAND_ID: usize = 2;

/// 托盘隐藏窗口句柄。
static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
/// 鼠标模式关闭时使用的图标句柄。
static INACTIVE_ICON: AtomicIsize = AtomicIsize::new(0);
/// 鼠标模式开启时使用的图标句柄。
static ACTIVE_ICON: AtomicIsize = AtomicIsize::new(0);

/// 创建托盘隐藏窗口并添加初始托盘图标。
pub fn init(instance: HINSTANCE) -> bool {
    let hwnd = unsafe { create_tray_window(instance) };
    if hwnd.is_null() {
        return false;
    }

    TRAY_HWND.store(hwnd as isize, Ordering::Relaxed);
    add_icon(hwnd)
}

/// 删除托盘项并释放两个动态创建的图标句柄。
pub fn shutdown() {
    let hwnd = tray_hwnd();
    if !hwnd.is_null() {
        delete_icon(hwnd);
    }

    destroy_stored_icon(&INACTIVE_ICON);
    destroy_stored_icon(&ACTIVE_ICON);
}

/// 根据鼠标模式状态更新托盘图标和悬停提示。
pub fn set_mouse_mode_active(active: bool) {
    let hwnd = tray_hwnd();
    if hwnd.is_null() {
        return;
    }

    // NIM_MODIFY 只需要提供要修改的图标和提示字段。
    let mut data = tray_icon_data(hwnd);
    data.uFlags = NIF_ICON | NIF_TIP;
    data.hIcon = if active {
        ACTIVE_ICON.load(Ordering::Relaxed) as HICON
    } else {
        INACTIVE_ICON.load(Ordering::Relaxed) as HICON
    };
    copy_wide_to_fixed(
        if active {
            "KMouse - 鼠标模式已启动"
        } else {
            "KMouse - 鼠标模式未启动"
        },
        &mut data.szTip,
    );

    unsafe {
        Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

/// 托盘隐藏窗口的消息处理函数。
unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Shell 将具体鼠标动作放在 lparam 中；这里只响应托盘图标右键释放。
    if msg == TRAY_CALLBACK_MESSAGE && lparam as u32 == WM_RBUTTONUP {
        show_exit_menu(hwnd);
        return 0;
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 注册窗口类并创建不可见的托盘消息窗口。
unsafe fn create_tray_window(instance: HINSTANCE) -> HWND {
    let class_name = wide("KMouseTrayWindow");
    let window_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(tray_proc),
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
        RegisterClassW(&window_class);

        // 窗口只接收消息，不需要样式、尺寸、父窗口或可见内容。
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("KMouse Tray").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            0 as HWND,
            0 as _,
            instance,
            ptr::null(),
        )
    }
}

/// 创建两种状态图标并将初始图标添加到通知区域。
fn add_icon(hwnd: HWND) -> bool {
    let inactive_icon = create_mouse_icon(false);
    let active_icon = create_mouse_icon(true);
    // 任一图标创建失败时释放已创建的另一个图标，避免半初始化资源泄漏。
    if inactive_icon.is_null() || active_icon.is_null() {
        unsafe {
            if !inactive_icon.is_null() {
                DestroyIcon(inactive_icon);
            }
            if !active_icon.is_null() {
                DestroyIcon(active_icon);
            }
        }
        return false;
    }

    INACTIVE_ICON.store(inactive_icon as isize, Ordering::Relaxed);
    ACTIVE_ICON.store(active_icon as isize, Ordering::Relaxed);

    let mut data = tray_icon_data(hwnd);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY_CALLBACK_MESSAGE;
    data.hIcon = inactive_icon;
    copy_wide_to_fixed("KMouse - 鼠标模式未启动", &mut data.szTip);

    // Shell_NotifyIconW 返回零表示资源管理器拒绝添加托盘项。
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) != 0 };
    if !added {
        destroy_stored_icon(&INACTIVE_ICON);
        destroy_stored_icon(&ACTIVE_ICON);
    }
    added
}

/// 从通知区域删除当前托盘项。
fn delete_icon(hwnd: HWND) {
    let data = tray_icon_data(hwnd);
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// 创建带有公共窗口句柄和图标编号的零初始化托盘数据结构。
fn tray_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = unsafe { MaybeUninit::<NOTIFYICONDATAW>::zeroed().assume_init() };
    data.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data
}

/// 在鼠标当前位置显示托盘右键菜单并执行所选命令。
fn show_exit_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }

        // 菜单文字根据当前状态动态显示相反操作。
        let active = crate::state::mouse_mode_active();
        let toggle_text = wide(if active {
            "关闭鼠标模式"
        } else {
            "启动鼠标模式"
        });
        AppendMenuW(
            menu,
            MF_STRING,
            TOGGLE_MODE_COMMAND_ID,
            toggle_text.as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());

        let exit_text = wide("退出");
        AppendMenuW(menu, MF_STRING, EXIT_COMMAND_ID, exit_text.as_ptr());

        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);

        // Windows 要求托盘菜单所属窗口成为前台窗口，否则菜单可能无法正确自动关闭。
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            ptr::null(),
        );
        DestroyMenu(menu);

        match command as usize {
            TOGGLE_MODE_COMMAND_ID => crate::state::set_mouse_mode(!active),
            EXIT_COMMAND_ID => PostQuitMessage(0),
            _ => {}
        }
    }
}

/// 使用 AND/XOR 单色位图动态创建鼠标形状托盘图标。
///
/// `active` 为 `true` 时生成高亮状态，为 `false` 时生成普通状态。
fn create_mouse_icon(active: bool) -> HICON {
    const SIZE: usize = 32;
    const ROW_BYTES: usize = SIZE / 8;

    // AND 掩码初始全透明，XOR 掩码用于决定不透明区域显示黑色或白色。
    let mut and_mask = [0xffu8; SIZE * ROW_BYTES];
    let mut xor_mask = [0u8; SIZE * ROW_BYTES];

    for y in 2..30 {
        for x in 8..24 {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            // 使用椭圆方程判断像素是否位于鼠标主体内部。
            let inside = dx * dx * 4 + dy * dy <= 15 * 15;
            if !inside {
                continue;
            }

            let boundary = dx * dx * 4 + dy * dy >= 13 * 13;
            let wheel_line = x == 15 && (5..11).contains(&y);
            let button_line = y == 12 && (9..23).contains(&x);
            set_mask_pixel(&mut and_mask, x, y, false);
            // 开启和关闭状态交换主体与轮廓亮度，形成明显视觉区分。
            let white = if active {
                boundary || wheel_line || button_line
            } else {
                !(boundary || wheel_line || button_line)
            };
            set_mask_pixel(&mut xor_mask, x, y, white);
        }
    }

    unsafe {
        CreateIcon(
            0 as HINSTANCE,
            SIZE as i32,
            SIZE as i32,
            1,
            1,
            and_mask.as_ptr(),
            xor_mask.as_ptr(),
        )
    }
}

/// 释放原子变量中保存的图标句柄，并将其重置为空。
fn destroy_stored_icon(storage: &AtomicIsize) {
    let icon = storage.swap(0, Ordering::Relaxed) as HICON;
    if !icon.is_null() {
        unsafe {
            DestroyIcon(icon);
        }
    }
}

/// 设置单色位图掩码中的单个像素。
fn set_mask_pixel(mask: &mut [u8], x: usize, y: usize, enabled: bool) {
    let index = y * 4 + x / 8;
    let bit = 0x80 >> (x % 8);
    if enabled {
        mask[index] |= bit;
    } else {
        mask[index] &= !bit;
    }
}

/// 读取托盘隐藏窗口句柄。
fn tray_hwnd() -> HWND {
    TRAY_HWND.load(Ordering::Relaxed) as HWND
}

/// 将 UTF-16 文本复制到 Windows 固定长度字符数组中。
fn copy_wide_to_fixed(text: &str, target: &mut [u16]) {
    for (destination, source) in target.iter_mut().zip(text.encode_utf16()) {
        *destination = source;
    }
}

/// 将 Rust 字符串转换为以 NUL 结尾的 UTF-16 Windows 字符串。
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
