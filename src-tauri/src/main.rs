// Impede a janela de console extra no Windows em builds de release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    esf_news_monitor_lib::run()
}
