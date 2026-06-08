//! 81 宫格选区和多显示器坐标计算。
//!
//! 所有 `Grid` 坐标均使用 Windows 虚拟桌面坐标。81 宫格由 9 行和 9 列组成，
//! 第一次数字输入选择行，第二次数字输入选择列。虚拟桌面可以覆盖多个显示器，
//! 且当副显示器位于主显示器左侧或上方时，其起点坐标可能为负数。

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[derive(Clone, Copy)]
/// 当前 81 宫格定位状态。
pub struct Grid {
    /// 81 宫格覆盖的虚拟桌面矩形范围。
    pub rect: RECT,
    /// 第一次输入选择的行号；尚未选择时为 `None`。
    pub row: Option<u8>,
    /// 第二次输入选择的列号；尚未选择时为 `None`。
    pub col: Option<u8>,
}

/// 获取覆盖所有显示器的虚拟桌面矩形。
pub fn virtual_screen_rect() -> RECT {
    unsafe {
        // Windows 分别提供虚拟桌面的起点和尺寸，需要组合成 RECT。
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

/// 返回 81 宫格中指定行列对应的单元格矩形。
///
/// 行列编号均为 `1..=9`，行从上到下，列从左到右。
pub fn cell_rect(rect: RECT, row: u8, col: u8) -> RECT {
    let row = row.saturating_sub(1) as i32;
    let col = col.saturating_sub(1) as i32;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    RECT {
        left: rect.left + width * col / 9,
        top: rect.top + height * row / 9,
        right: rect.left + width * (col + 1) / 9,
        bottom: rect.top + height * (row + 1) / 9,
    }
}

/// 返回指定行覆盖的完整横向矩形。
pub fn row_rect(rect: RECT, row: u8) -> RECT {
    let row = row.saturating_sub(1) as i32;
    let height = rect.bottom - rect.top;

    RECT {
        left: rect.left,
        top: rect.top + height * row / 9,
        right: rect.right,
        bottom: rect.top + height * (row + 1) / 9,
    }
}

/// 返回当前输入阶段对应的鼠标目标区域。
pub fn target_rect(grid: Grid) -> RECT {
    match (grid.row, grid.col) {
        (Some(row), Some(col)) => cell_rect(grid.rect, row, col),
        (Some(row), None) => row_rect(grid.rect, row),
        _ => grid.rect,
    }
}

/// 将虚拟桌面坐标转换为覆盖窗口客户区坐标。
///
/// 覆盖窗口客户区从 `(0, 0)` 开始，因此需要减去虚拟桌面的左上角偏移。
pub fn to_client_rect(rect: RECT, screen: RECT) -> RECT {
    RECT {
        left: rect.left - screen.left,
        top: rect.top - screen.top,
        right: rect.right - screen.left,
        bottom: rect.bottom - screen.top,
    }
}
