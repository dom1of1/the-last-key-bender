use eframe::egui;

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
