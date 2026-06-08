//! 全局低级键盘钩子与按键动作分发。
//!
//! 本模块接收系统键盘事件，在鼠标模式启用时将指定按键转换为鼠标动作。
//! 返回 `1` 表示吞掉事件，调用 `CallNextHookEx` 则表示将事件继续交给系统和前台程序。

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

/// 安装当前桌面会话的低级键盘钩子。
///
/// 返回空句柄表示安装失败。钩子的生命周期依赖主线程持续运行 Windows 消息循环。
pub fn install(instance: HINSTANCE) -> HHOOK {
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), instance, 0) }
}

/// 卸载已经安装的键盘钩子。
pub fn uninstall(hook: HHOOK) {
    unsafe {
        UnhookWindowsHookEx(hook);
    }
}

/// Windows 调用的低级键盘钩子回调。
///
/// `code` 小于零时必须无条件放行；`wparam` 表示按下或释放事件；
/// `lparam` 指向包含虚拟键码等信息的 `KBDLLHOOKSTRUCT`。
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    // 将 Windows 提供的原始指针读取为键盘事件结构，并归一化按下/释放状态。
    let kb = unsafe { *(lparam as *const KBDLLHOOKSTRUCT) };
    let vk = kb.vkCode;
    let pressed = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;
    let released = wparam as u32 == WM_KEYUP || wparam as u32 == WM_SYSKEYUP;

    // Ctrl+M 是全局总开关，必须在检查 mouse_mode 之前处理，
    // 才能在鼠标模式关闭时重新开启，并在开启时再次关闭。
    if pressed && vk == VK_M && ctrl_is_down() {
        state::set_mouse_mode(!state::mouse_mode_active());
        return 1;
    }

    // 状态未初始化或鼠标模式关闭时，不拦截任何普通按键。
    let Some(mut app) = state::lock() else {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    };

    if !app.mouse_mode {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    // 放行所有 Win 组合键，例如 Win+D。否则映射为中键的 D 会破坏系统快捷键。
    if win_is_down() {
        return unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) };
    }

    // 只将主键盘 `/` 识别为 DPI 键，不处理小键盘除号。
    let dpi_key = vk == VK_OEM_2;

    // 按下和释放需要分别处理，鼠标按钮才能正确支持按住拖动。
    let handled = if pressed {
        handle_pressed(vk, dpi_key, &mut app)
    } else if released {
        handle_released(vk, dpi_key, &mut app)
    } else {
        false
    };

    // 已映射事件返回 1，防止原始键盘字符继续传给前台应用。
    if handled {
        return 1;
    }

    unsafe { CallNextHookEx(0 as HHOOK, code, wparam, lparam) }
}

/// 处理按键按下事件。
///
/// 返回 `true` 表示该按键已被 KMouse 消费，应由钩子吞掉原始事件。
fn handle_pressed(vk: u32, dpi_key: bool, app: &mut AppState) -> bool {
    // 九宫格命令优先于普通映射，确保数字与 Enter 只控制定位流程。
    if let Some(grid) = app.grid {
        if (VK_1..=VK_9).contains(&vk) {
            choose_grid_cell(app, grid, (vk - VK_1 + 1) as u8);
            return true;
        }

        // Enter 确认当前区域中心并退出九宫格，但不会关闭整个鼠标模式。
        if vk == VK_RETURN as u32 {
            mouse::move_to_rect_center(grid.rect);
            app.grid = None;
            overlay::hide();
            println!("grid mode: off");
            return true;
        }
    }

    // DPI 是软件移动步长，不修改物理鼠标或系统鼠标设置。
    if dpi_key {
        let dpi = app.cycle_dpi();
        println!("dpi: {dpi}");
        return true;
    }

    // 普通鼠标模式键位映射。
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

/// 处理按键释放事件。
///
/// 滚轮层在释放 Z/X 时关闭；鼠标按钮映射在释放时发送对应的按钮抬起事件。
fn handle_released(vk: u32, dpi_key: bool, app: &mut AppState) -> bool {
    // DPI 已在按下时切换，释放事件仅需吞掉，防止 `/` 输入到前台程序。
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

/// 根据方向向量执行移动或滚轮动作。
fn arrow_action(dx: i32, dy: i32, app: &AppState) {
    // 垂直滚轮层优先，仅响应上下方向。
    if app.vertical_wheel {
        if dy != 0 {
            mouse::wheel(if dy < 0 { WHEEL_DELTA } else { -WHEEL_DELTA }, false);
        }
        return;
    }

    // 水平滚轮层仅响应左右方向。
    if app.horizontal_wheel {
        if dx != 0 {
            mouse::wheel(if dx < 0 { -WHEEL_DELTA } else { WHEEL_DELTA }, true);
        }
        return;
    }

    // 未启用滚轮层时，按当前 DPI 计算相对鼠标移动量。
    let step = app.movement_step();
    mouse::move_relative(dx * step, dy * step);
}

/// 从整个虚拟桌面开始新的九宫格定位流程。
fn start_grid(app: &mut AppState) {
    app.grid = Some(Grid {
        rect: virtual_screen_rect(),
        depth: 0,
    });
    overlay::show();
}

/// 选择当前九宫格中的一个区域并进入下一层。
fn choose_grid_cell(app: &mut AppState, grid: Grid, number: u8) {
    // 达到最大深度后忽略数字键，等待用户按 Enter 确认。
    if grid.depth >= MAX_GRID_DEPTH {
        return;
    }

    // 鼠标立即移动到所选格中心，覆盖窗口随后只绘制该区域内的新九宫格。
    let next = cell_rect(grid.rect, number);
    mouse::move_to_rect_center(next);

    app.grid = Some(Grid {
        rect: next,
        depth: grid.depth + 1,
    });
    overlay::show();
}

/// 判断 Ctrl 修饰键当前是否按下。
fn ctrl_is_down() -> bool {
    unsafe { GetKeyState(VK_CONTROL as i32) < 0 }
}

/// 判断左或右 Windows 键当前是否按下。
fn win_is_down() -> bool {
    unsafe { GetKeyState(VK_LWIN as i32) < 0 || GetKeyState(VK_RWIN as i32) < 0 }
}
