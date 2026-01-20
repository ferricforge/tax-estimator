use egui::{Response, Ui};

/// A reusable currency input field with label, using a grid for alignment
pub fn currency_field(ui: &mut Ui, label: &str, value: &mut String) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_space(10.0);
        ui.label("$");
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(120.0)
                .hint_text("0.00"),
        )
    })
    .inner
}
