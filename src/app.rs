use eframe::egui;

/// All of the app's state lives here. egui redraws the whole UI every frame
/// from this state ("immediate mode"), so there are no widget objects to keep in sync.
#[derive(Default)]
pub struct PianoApp {}

impl eframe::App for PianoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("The Last Key Bender");
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
        });
    }
}
