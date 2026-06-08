use std::mem;

use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN,
    MOUSEEVENTF_XUP, MOUSEINPUT, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{XBUTTON1, XBUTTON2};

use crate::grid::virtual_screen_rect;

pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButton {
    fn down_flags(&self) -> (u32, u32) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTDOWN, 0),
            Self::Right => (MOUSEEVENTF_RIGHTDOWN, 0),
            Self::Middle => (MOUSEEVENTF_MIDDLEDOWN, 0),
            Self::Back => (MOUSEEVENTF_XDOWN, XBUTTON1 as u32),
            Self::Forward => (MOUSEEVENTF_XDOWN, XBUTTON2 as u32),
        }
    }

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

pub fn move_relative(dx: i32, dy: i32) {
    send_mouse(MOUSEEVENTF_MOVE, dx, dy, 0);
}

pub fn move_to_rect_center(rect: RECT) {
    let x = (rect.left + rect.right) / 2;
    let y = (rect.top + rect.bottom) / 2;
    let screen = virtual_screen_rect();
    let width = (screen.right - screen.left).max(1);
    let height = (screen.bottom - screen.top).max(1);

    let abs_x = ((x - screen.left) * 65535) / width;
    let abs_y = ((y - screen.top) * 65535) / height;
    send_mouse(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE, abs_x, abs_y, 0);
}

pub fn button_down(button: MouseButton) {
    let (flags, data) = button.down_flags();
    send_mouse(flags, 0, 0, data);
}

pub fn button_up(button: MouseButton) {
    let (flags, data) = button.up_flags();
    send_mouse(flags, 0, 0, data);
}

pub fn wheel(amount: i32, horizontal: bool) {
    let flag = if horizontal {
        MOUSEEVENTF_HWHEEL
    } else {
        MOUSEEVENTF_WHEEL
    };
    send_mouse(flag, 0, 0, amount as u32);
}

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

    unsafe {
        SendInput(1, &input, mem::size_of::<INPUT>() as i32);
    }
}
