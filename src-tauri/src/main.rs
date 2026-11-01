// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if !floatingbar_lib::handle_cli() { floatingbar_lib::run() }
}
