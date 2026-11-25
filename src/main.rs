// No console window behind the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod keymap;
mod piano_view;
mod sampler;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("The Last Key Bender")
            .with_inner_size([1100.0, 420.0]),
        ..Default::default()
    };

    eframe::run_native(
        "The Last Key Bender",
        options,
        Box::new(|_cc| Ok(Box::new(app::PianoApp::new()))),
    )
}
