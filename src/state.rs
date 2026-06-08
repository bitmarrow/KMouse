use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::config::{BASE_MOVE, DEFAULT_DPI_INDEX, DPI_LEVELS};
use crate::grid::Grid;
use crate::overlay;
use crate::tray;

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

pub struct AppState {
    pub mouse_mode: bool,
    pub dpi_index: usize,
    pub vertical_wheel: bool,
    pub horizontal_wheel: bool,
    pub grid: Option<Grid>,
}

impl AppState {
    fn new() -> Self {
        Self {
            mouse_mode: false,
            dpi_index: DEFAULT_DPI_INDEX,
            vertical_wheel: false,
            horizontal_wheel: false,
            grid: None,
        }
    }

    pub fn cycle_dpi(&mut self) -> i32 {
        self.dpi_index = (self.dpi_index + 1) % DPI_LEVELS.len();
        DPI_LEVELS[self.dpi_index]
    }

    pub fn movement_step(&self) -> i32 {
        BASE_MOVE * DPI_LEVELS[self.dpi_index] / 400
    }
}

pub fn init() {
    STATE.set(Mutex::new(AppState::new())).ok();
}

pub fn lock() -> Option<MutexGuard<'static, AppState>> {
    STATE
        .get()
        .map(|state| state.lock().expect("state mutex poisoned"))
}

pub fn set_mouse_mode(enabled: bool) {
    if let Some(mut app) = lock() {
        app.mouse_mode = enabled;
        app.grid = None;
    }

    overlay::hide();
    tray::set_mouse_mode_active(enabled);
    println!("mouse mode: {}", if enabled { "on" } else { "off" });
}

pub fn mouse_mode_active() -> bool {
    lock().is_some_and(|app| app.mouse_mode)
}
