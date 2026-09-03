// 防止 Windows release 构建弹出额外的控制台窗口，此行勿删！
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    markbox_lib::run()
}
