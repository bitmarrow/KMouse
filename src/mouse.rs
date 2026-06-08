//! 使用 Windows `SendInput` 生成鼠标事件。
//!
//! 本模块统一封装相对移动、绝对定位、按钮按下/释放和滚轮输入，
//! 将不安全的 Win32 调用与键盘映射逻辑隔离。

use std::mem;

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{XBUTTON1, XBUTTON2};

use crate::grid::virtual_screen_rect;

/// KMouse 支持映射的鼠标按钮。
#[derive(Clone, Copy)]
pub enum MouseButton {
    /// 鼠标左键。
    Left,
    /// 鼠标右键。
    Right,
    /// 鼠标中键。
    Middle,
    /// 后退侧键，对应 XBUTTON1。
    Back,
    /// 前进侧键，对应 XBUTTON2。
    Forward,
}

impl MouseButton {
    /// 所有鼠标按钮，顺序与 `index` 返回的索引保持一致。
    pub const ALL: [Self; 5] = [
        Self::Left,
        Self::Right,
        Self::Middle,
        Self::Back,
        Self::Forward,
    ];

    /// 返回按钮在应用按住状态数组中的固定索引。
    pub fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Middle => 2,
            Self::Back => 3,
            Self::Forward => 4,
        }
    }

    /// 返回按钮按下事件所需的 SendInput 标志和附加数据。
    fn down_flags(&self) -> (u32, u32) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTDOWN, 0),
            Self::Right => (MOUSEEVENTF_RIGHTDOWN, 0),
            Self::Middle => (MOUSEEVENTF_MIDDLEDOWN, 0),
            Self::Back => (MOUSEEVENTF_XDOWN, XBUTTON1 as u32),
            Self::Forward => (MOUSEEVENTF_XDOWN, XBUTTON2 as u32),
        }
    }

    /// 返回按钮释放事件所需的 SendInput 标志和附加数据。
    fn up_flags(&self) -> (u32, u32) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTUP, 0),
            Self::Right => (MOUSEEVENTF_RIGHTUP, 0),
            Self::Middle => (MOUSEEVENTF_MIDDLEUP, 0),
            Self::Back => (MOUSEEVENTF_XUP, XBUTTON1 as u32),
            Self::Forward => (MOUSEEVENTF_XUP, XBUTTON2 as u32),
        }
    }
}

/// 发送相对鼠标移动事件。
///
/// Windows 可能对相对移动应用系统鼠标加速度，因此最终像素距离不一定严格等于参数。
pub fn move_relative(dx: i32, dy: i32) {
    send_mouse(MOUSEEVENTF_MOVE, dx, dy, 0);
}

/// 将鼠标绝对定位到虚拟桌面矩形的中心。
pub fn move_to_rect_center(rect: RECT) {
    // 先计算矩形中心和虚拟桌面尺寸，宽高最小取 1 以避免除零。
    let x = (rect.left + rect.right) / 2;
    let y = (rect.top + rect.bottom) / 2;
    let screen = virtual_screen_rect();
    let width = (screen.right - screen.left).max(1);
    let height = (screen.bottom - screen.top).max(1);

    // SendInput 的绝对坐标范围为 0..65535，而不是屏幕物理像素。
    let abs_x = ((x - screen.left) * 65535) / width;
    let abs_y = ((y - screen.top) * 65535) / height;
    send_mouse(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        abs_x,
        abs_y,
        0,
    );
}

/// 发送指定鼠标按钮的按下事件。
pub fn button_down(button: MouseButton) {
    let (flags, data) = button.down_flags();
    send_mouse(flags, 0, 0, data);
}

/// 发送指定鼠标按钮的释放事件。
pub fn button_up(button: MouseButton) {
    let (flags, data) = button.up_flags();
    send_mouse(flags, 0, 0, data);
}

/// 发送滚轮事件。
///
/// `horizontal` 为 `true` 时发送水平滚轮，否则发送垂直滚轮；
/// `amount` 的正负值决定滚动方向。
pub fn wheel(amount: i32, horizontal: bool) {
    let flag = if horizontal {
        MOUSEEVENTF_HWHEEL
    } else {
        MOUSEEVENTF_WHEEL
    };
    send_mouse(flag, 0, 0, amount as u32);
}

/// 构造并发送单个底层鼠标输入事件。
///
/// `mouse_data` 用于承载滚轮增量或侧键编号，其含义由 `flags` 决定。
fn send_mouse(flags: u32, dx: i32, dy: i32, mouse_data: u32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: mouse_data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    // INPUT 结构已完整初始化，长度参数用于让 Windows 校验调用方结构版本。
    unsafe {
        SendInput(1, &input, mem::size_of::<INPUT>() as i32);
    }
}
