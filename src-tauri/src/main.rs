//! Z Desktop 桌面应用入口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    z_desktop_lib::run()
}
