#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! KMouse 程序入口。
//!
//! 本模块负责按照正确顺序初始化进程级资源，并运行 Windows 消息循环。
//! 具体的键盘钩子、鼠标事件、81 宫格绘制、共享状态和托盘功能分别由独立模块实现。

mod config;
mod grid;
mod keyboard;
mod mouse;
mod overlay;
mod state;
mod tray;

use std::mem::MaybeUninit;
use std::ptr;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

fn main() {
    // 覆盖窗口和 SendInput 都使用物理/虚拟桌面坐标。提前启用每显示器 DPI 感知，
    // 可以避免 Windows 自动缩放窗口坐标后造成 81 宫格与鼠标位置不一致。
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    // 共享状态必须最先初始化，因为后续创建的窗口过程和键盘钩子都可能访问它。
    state::init();

    // 当前模块句柄用于注册窗口类和安装全局低级键盘钩子。
    let module = unsafe { GetModuleHandleW(ptr::null()) };

    // 覆盖窗口负责显示 81 宫格；创建失败时程序无法提供完整功能，因此直接退出。
    if !overlay::init(module as _) {
        eprintln!("failed to create overlay window");
        return;
    }

    // 托盘是无控制台程序的主要状态指示与退出入口，创建失败时不继续后台运行。
    if !tray::init(module as _) {
        eprintln!("failed to create tray icon");
        return;
    }

    // 安装键盘钩子后才能接收 Ctrl+M 和鼠标模式按键。
    let hook = keyboard::install(module as _);
    if hook.is_null() {
        eprintln!("failed to install keyboard hook");
        tray::shutdown();
        return;
    }

    println!("KMouse started");
    println!("Ctrl+M: toggle mouse mode");
    println!("Arrows move, A/S/D/Q/W click, Z/X wheel layer, / DPI, F grid");

    // 低级键盘钩子、托盘窗口和覆盖窗口均依赖消息循环分发事件。
    unsafe {
        let mut msg = MaybeUninit::<MSG>::zeroed();
        while GetMessageW(msg.as_mut_ptr(), 0 as HWND, 0, 0) > 0 {
            let msg = msg.assume_init_ref();
            TranslateMessage(msg);
            DispatchMessageW(msg);
        }
    }

    // 消息循环结束后按与初始化相反的顺序释放系统资源。
    tray::shutdown();
    keyboard::uninstall(hook);
}
