use windows_sys::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_DOWN, VK_LEFT, VK_LWIN, VK_RETURN, VK_RIGHT, VK_RWIN, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::config::{
    MAX_GRID_DEPTH, VK_1, VK_9, VK_A, VK_D, VK_F, VK_M, VK_OEM_2, VK_Q, VK_S, VK_W, VK_X, VK_Z,
    WHEEL_DELTA,
};
use crate::grid::{Grid, cell_rect, virtual_screen_rect};
use crate::mouse::{self, MouseButton};
use crate::overlay;
use crate::state::{self, AppState};

pub fn install(instance: HINSTANCE) -> HHOOK {
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), instance, 0) }
}

pub fn uninstall(hook: HHOOK) {
    unsafe {
        UnhookWindowsHookEx(hook);
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    let kb = unsafe { *(lparam as *const KBDLLHOOKSTRUCT) };
    let vk = kb.vkCode;
    let pressed = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;
    let released = wparam as u32 == WM_KEYUP || wparam as u32 == WM_SYSKEYUP;

    if pressed && vk == VK_M && ctrl_is_down() {
        state::set_mouse_mode(!state::mouse_mode_active());
        return 1;
    }

    let Some(mut app) = state::lock() else {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    };

    if !app.mouse_mode {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    if win_is_down() {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    let dpi_key = vk == VK_OEM_2;
    let handled = if pressed {
        handle_pressed(vk, dpi_key, &mut app)
    } else if released {
        handle_released(vk, dpi_key, &mut app)
    } else {
        false
    };

    if handled {
        return 1;
    }

    unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) }
}

fn handle_pressed(vk: u32, dpi_key: bool, app: &mut AppState) -> bool {
    if let Some(grid) = app.grid {
        if (VK_1..=VK_9).contains(&vk) {
            choose_grid_cell(app, grid, (vk - VK_1 + 1) as u8);
            return true;
        }

        if vk == VK_RETURN as u32 {
            mouse::move_to_rect_center(grid.rect);
            app.grid = None;
            overlay::hide();
            println!("grid mode: off");
            return true;
        }
    }

    if dpi_key {
        let dpi = app.cycle_dpi();
        println!("dpi: {dpi}");
        return true;
    }

    match vk {
        VK_F => {
            start_grid(app);
            true
        }
        VK_Z => {
            app.vertical_wheel = true;
            true
        }
        VK_X => {
            app.horizontal_wheel = true;
            true
        }
        vk if vk == VK_UP as u32 => {
            arrow_action(0, -1, app);
            true
        }
        vk if vk == VK_DOWN as u32 => {
            arrow_action(0, 1, app);
            true
        }
        vk if vk == VK_LEFT as u32 => {
            arrow_action(-1, 0, app);
            true
        }
        vk if vk == VK_RIGHT as u32 => {
            arrow_action(1, 0, app);
            true
        }
        VK_A => {
            mouse::button_down(MouseButton::Left);
            true
        }
        VK_S => {
            mouse::button_down(MouseButton::Right);
            true
        }
        VK_D => {
            mouse::button_down(MouseButton::Middle);
            true
        }
        VK_Q => {
            mouse::button_down(MouseButton::Back);
            true
        }
        VK_W => {
            mouse::button_down(MouseButton::Forward);
            true
        }
        _ => false,
    }
}

fn handle_released(vk: u32, dpi_key: bool, app: &mut AppState) -> bool {
    if dpi_key {
        return true;
    }

    match vk {
        VK_Z => {
            app.vertical_wheel = false;
            true
        }
        VK_X => {
            app.horizontal_wheel = false;
            true
        }
        VK_A => {
            mouse::button_up(MouseButton::Left);
            true
        }
        VK_S => {
            mouse::button_up(MouseButton::Right);
            true
        }
        VK_D => {
            mouse::button_up(MouseButton::Middle);
            true
        }
        VK_Q => {
            mouse::button_up(MouseButton::Back);
            true
        }
        VK_W => {
            mouse::button_up(MouseButton::Forward);
            true
        }
        _ => false,
    }
}

fn arrow_action(dx: i32, dy: i32, app: &AppState) {
    if app.vertical_wheel {
        if dy != 0 {
            mouse::wheel(if dy < 0 { WHEEL_DELTA } else { -WHEEL_DELTA }, false);
        }
        return;
    }

    if app.horizontal_wheel {
        if dx != 0 {
            mouse::wheel(if dx < 0 { -WHEEL_DELTA } else { WHEEL_DELTA }, true);
        }
        return;
    }

    let step = app.movement_step();
    mouse::move_relative(dx * step, dy * step);
}

fn start_grid(app: &mut AppState) {
    app.grid = Some(Grid {
        rect: virtual_screen_rect(),
        depth: 0,
    });
    overlay::show();
}

fn choose_grid_cell(app: &mut AppState, grid: Grid, number: u8) {
    if grid.depth >= MAX_GRID_DEPTH {
        return;
    }

    let next = cell_rect(grid.rect, number);
    mouse::move_to_rect_center(next);

    app.grid = Some(Grid {
        rect: next,
        depth: grid.depth + 1,
    });
    overlay::show();
}

fn ctrl_is_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL as i32) < 0 }
}

fn win_is_down() -> bool {
    unsafe { GetKeyState(VK_LWIN as i32) < 0 || GetKeyState(VK_RWIN as i32) < 0 }
}
