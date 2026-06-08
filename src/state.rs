//! 跨模块共享的运行状态。
//!
//! 键盘钩子会更新状态，覆盖窗口绘制回调和托盘菜单会读取状态。
//! 使用 `OnceLock<Mutex<_>>` 保证状态只初始化一次，并安全协调多个系统回调。

use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::config::{BASE_MOVE, DEFAULT_DPI_INDEX, DPI_LEVELS};
use crate::grid::Grid;
use crate::mouse::{self, MouseButton};
use crate::overlay;
use crate::tray;

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

/// 全局应用状态，只允许在持有 `STATE` 互斥锁时读写。
pub struct AppState {
    /// 是否启用鼠标模式；关闭时所有普通映射键都会放行。
    pub mouse_mode: bool,
    /// 当前虚拟 DPI 在 `DPI_LEVELS` 中的索引。
    pub dpi_index: usize,
    /// Z 键是否按住；启用时上下方向键转换为垂直滚轮。
    pub vertical_wheel: bool,
    /// X 键是否按住；启用时左右方向键转换为水平滚轮。
    pub horizontal_wheel: bool,
    /// 当前 81 宫格选区；`None` 表示未处于定位模式。
    pub grid: Option<Grid>,
    /// 数字键是否仍处于按下状态，用于阻止键盘自动重复同时选择行和列。
    pub grid_number_down: bool,
    /// 五种鼠标按钮当前是否已发送按下事件，用于过滤键盘自动重复。
    pub mouse_buttons_down: [bool; 5],
}

impl AppState {
    /// 创建具有默认 DPI 且所有模式均关闭的初始状态。
    fn new() -> Self {
        Self {
            mouse_mode: false,
            dpi_index: DEFAULT_DPI_INDEX,
            vertical_wheel: false,
            horizontal_wheel: false,
            grid: None,
            grid_number_down: false,
            mouse_buttons_down: [false; 5],
        }
    }

    /// 循环切换至下一个 DPI 档位，并返回切换后的数值。
    pub fn cycle_dpi(&mut self) -> i32 {
        self.dpi_index = (self.dpi_index + 1) % DPI_LEVELS.len();
        DPI_LEVELS[self.dpi_index]
    }

    /// 按当前 DPI 比例计算单次方向键移动量。
    pub fn movement_step(&self) -> i32 {
        BASE_MOVE * DPI_LEVELS[self.dpi_index] / 400
    }
}

/// 初始化全局应用状态；程序启动过程中只应调用一次。
pub fn init() {
    STATE.set(Mutex::new(AppState::new())).ok();
}

/// 获取全局状态锁。
///
/// 返回 `None` 表示状态尚未初始化。调用方应尽量缩短持锁时间，避免阻塞系统回调。
pub fn lock() -> Option<MutexGuard<'static, AppState>> {
    STATE
        .get()
        .map(|state| state.lock().expect("state mutex poisoned"))
}

/// 开启或关闭鼠标模式，并同步清理 81 宫格和更新托盘图标。
pub fn set_mouse_mode(enabled: bool) {
    let mut buttons_to_release = [false; 5];
    // 先在短持锁区间内更新纯状态，再执行可能触发 Windows 消息的 UI 操作。
    if let Some(mut app) = lock() {
        app.mouse_mode = enabled;
        app.grid = None;
        app.grid_number_down = false;
        buttons_to_release = app.mouse_buttons_down;
        app.mouse_buttons_down = [false; 5];
    }

    // 模式切换时释放仍按住的虚拟鼠标按钮，避免按钮在系统中保持按下状态。
    for button in MouseButton::ALL {
        if buttons_to_release[button.index()] {
            mouse::button_up(button);
        }
    }

    overlay::hide();
    tray::set_mouse_mode_active(enabled);
    println!("mouse mode: {}", if enabled { "on" } else { "off" });
}

/// 查询当前是否启用了鼠标模式。
pub fn mouse_mode_active() -> bool {
    lock().is_some_and(|app| app.mouse_mode)
}
