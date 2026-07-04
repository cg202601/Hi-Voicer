#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if hi_voicer_lib::run_cli_mode() {
        return;
    }
    hi_voicer_lib::run();
}
