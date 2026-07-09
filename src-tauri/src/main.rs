// Windows'ta ek konsol penceresi açılmasını engeller (Linux'ta etkisizdir).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    amorfly_ai_lib::run();
}
