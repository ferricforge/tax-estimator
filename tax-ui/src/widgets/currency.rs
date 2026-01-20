use egui::{Response, Ui};

/// A reusable currency input field with label
pub fn currency_field(ui: &mut Ui, label: &str, value: &mut String) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_space(10.0);
        ui.label("$");
        let response = ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(120.0)
                .hint_text("0.00"),
        );
        response
    })
    .inner
}

/// Currency field with validation feedback
pub fn currency_field_validated(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    error: Option<&str>,
) -> Response {
    ui.vertical(|ui| {
        let response = currency_field(ui, label, value);

        if let Some(err) = error {
            ui.colored_label(egui::Color32::RED, err);
        }

        response
    })
    .inner
}
