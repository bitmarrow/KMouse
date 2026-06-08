//! 九宫格选区和多显示器坐标计算。
//!
//! 所有 `Grid` 坐标均使用 Windows 虚拟桌面坐标。虚拟桌面可以覆盖多个显示器，
//! 且当副显示器位于主显示器左侧或上方时，其起点坐标可能为负数。

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

#[derive(Clone, Copy)]
/// 当前九宫格定位状态。
pub struct Grid {
    /// 当前活动选区在虚拟桌面中的矩形范围。
    pub rect: RECT,
    /// 已完成的细分次数，用于判断是否达到最大深度。
    pub depth: u8,
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

/// 将矩形平均划分为九宫格，并返回指定数字对应的子矩形。
///
/// 数字按从左到右、从上到下排列，即 1 位于左上角，9 位于右下角。
pub fn cell_rect(rect: RECT, number: u8) -> RECT {
    // 将 1..=9 转换为 0..=8，便于通过除法和取模计算行列。
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
