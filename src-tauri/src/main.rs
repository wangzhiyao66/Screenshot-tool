// 打包发布时隐藏 Windows 的控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shotly_lib::run()
}
