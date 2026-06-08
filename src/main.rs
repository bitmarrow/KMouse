#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    state::init();

    let module = unsafe { GetModuleHandleW(ptr::null()) };
    if !overlay::init(module as _) {
        eprintln!("failed to create overlay window");
        return;
    }

    if !tray::init(module as _) {
        eprintln!("failed to create tray icon");
        return;
    }

    let hook = keyboard::install(module as _);
    if hook.is_null() {
        eprintln!("failed to install keyboard hook");
        tray::shutdown();
        return;
    }

    println!("KMouse started");
    println!("Ctrl+M: toggle mouse mode");
    println!("Arrows move, A/S/D/Q/W click, Z/X wheel layer, / DPI, F grid");

    unsafe {
        let mut msg = MaybeUninit::<MSG>::zeroed();
        while GetMessageW(msg.as_mut_ptr(), 0 as HWND, 0, 0) > 0 {
            let msg = msg.assume_init_ref();
            TranslateMessage(msg);
            DispatchMessageW(msg);
        }
    }

    tray::shutdown();
    keyboard::uninstall(hook);
}
