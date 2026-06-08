//! KMouse 的键位映射和行为参数。
//!
//! 将可调参数集中在本模块，可以在不修改事件处理流程的情况下调整快捷键、
//! 鼠标移动速度、DPI 档位、滚轮步长和九宫格最大细分深度。

// 字母键对应的 Windows 虚拟键码。
pub const VK_A: u32 = 0x41;
pub const VK_D: u32 = 0x44;
pub const VK_F: u32 = 0x46;
pub const VK_M: u32 = 0x4D;
pub const VK_Q: u32 = 0x51;
pub const VK_S: u32 = 0x53;
pub const VK_W: u32 = 0x57;
pub const VK_X: u32 = 0x58;
pub const VK_Z: u32 = 0x5A;

// 九宫格数字选择范围。
pub const VK_1: u32 = 0x31;
pub const VK_9: u32 = 0x39;

/// 主键盘 `/` 对应的 Windows 虚拟键码，用于切换虚拟 DPI。
pub const VK_OEM_2: u32 = 0xBF;

/// 400 DPI 档位下，每次方向键事件对应的基础移动量。
pub const BASE_MOVE: i32 = 18;

/// `/` 键循环切换的虚拟 DPI 档位。
pub const DPI_LEVELS: [i32; 5] = [200, 400, 800, 1600, 3200];

/// 默认 DPI 在 `DPI_LEVELS` 中的索引；索引 1 对应 400 DPI。
pub const DEFAULT_DPI_INDEX: usize = 1;

/// 单次标准滚轮事件使用的增量值。
pub const WHEEL_DELTA: i32 = 120;

/// 九宫格允许的最大细分层数，防止选区无限缩小。
pub const MAX_GRID_DEPTH: u8 = 4;
