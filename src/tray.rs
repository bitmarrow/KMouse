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

const TRAY_ICON_ID: u32 = 1;
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const EXIT_COMMAND_ID: usize = 1;
const TOGGLE_MODE_COMMAND_ID: usize = 2;

static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
static INACTIVE_ICON: AtomicIsize = AtomicIsize::new(0);
static ACTIVE_ICON: AtomicIsize = AtomicIsize::new(0);

pub fn init(instance: HINSTANCE) -> bool {
    let hwnd = unsafe { create_tray_window(instance) };
    if hwnd.is_null() {
        return false;
    }

    TRAY_HWND.store(hwnd as isize, Ordering::Relaxed);
    add_icon(hwnd)
}

pub fn shutdown() {
    let hwnd = tray_hwnd();
    if !hwnd.is_null() {
        delete_icon(hwnd);
    }

    destroy_stored_icon(&INACTIVE_ICON);
    destroy_stored_icon(&ACTIVE_ICON);
}

pub fn set_mouse_mode_active(active: bool) {
    let hwnd = tray_hwnd();
    if hwnd.is_null() {
        return;
    }

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

unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == TRAY_CALLBACK_MESSAGE && lparam as u32 == WM_RBUTTONUP {
        show_exit_menu(hwnd);
        return 0;
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

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

fn add_icon(hwnd: HWND) -> bool {
    let inactive_icon = create_mouse_icon(false);
    let active_icon = create_mouse_icon(true);
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

    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) != 0 };
    if !added {
        destroy_stored_icon(&INACTIVE_ICON);
        destroy_stored_icon(&ACTIVE_ICON);
    }
    added
}

fn delete_icon(hwnd: HWND) {
    let data = tray_icon_data(hwnd);
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn tray_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = unsafe { MaybeUninit::<NOTIFYICONDATAW>::zeroed().assume_init() };
    data.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data
}

fn show_exit_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }

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

fn create_mouse_icon(active: bool) -> HICON {
    const SIZE: usize = 32;
    const ROW_BYTES: usize = SIZE / 8;

    let mut and_mask = [0xffu8; SIZE * ROW_BYTES];
    let mut xor_mask = [0u8; SIZE * ROW_BYTES];

    for y in 2..30 {
        for x in 8..24 {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            let inside = dx * dx * 4 + dy * dy <= 15 * 15;
            if !inside {
                continue;
            }

            let boundary = dx * dx * 4 + dy * dy >= 13 * 13;
            let wheel_line = x == 15 && (5..11).contains(&y);
            let button_line = y == 12 && (9..23).contains(&x);
            set_mask_pixel(&mut and_mask, x, y, false);
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

fn destroy_stored_icon(storage: &AtomicIsize) {
    let icon = storage.swap(0, Ordering::Relaxed) as HICON;
    if !icon.is_null() {
        unsafe {
            DestroyIcon(icon);
        }
    }
}

fn set_mask_pixel(mask: &mut [u8], x: usize, y: usize, enabled: bool) {
    let index = y * 4 + x / 8;
    let bit = 0x80 >> (x % 8);
    if enabled {
        mask[index] |= bit;
    } else {
        mask[index] &= !bit;
    }
}

fn tray_hwnd() -> HWND {
    TRAY_HWND.load(Ordering::Relaxed) as HWND
}

fn copy_wide_to_fixed(text: &str, target: &mut [u16]) {
    for (destination, source) in target.iter_mut().zip(text.encode_utf16()) {
        *destination = source;
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
