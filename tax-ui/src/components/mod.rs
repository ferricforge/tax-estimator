mod buttons;
mod clear_confirm;
mod dialogs;
mod edit_menu;
mod estimate_form;
mod estimate_selector;
mod file_menu;
mod form_rows;
mod inputs;
mod preferences;
mod qbi_form;
mod recent_labels;
mod results_form;
mod se_worksheet_form;
mod theme;
mod validation_view;
mod window;
mod window_geometry;
mod window_preferences;

pub use buttons::make_button;
pub use clear_confirm::confirm_clear;
pub use dialogs::{ErrorDialog, InfoDialog, show_err};
pub use estimate_form::EstimatedIncomeForm;
pub use estimate_selector::EstimateSelector;
#[cfg(target_os = "macos")]
pub use file_menu::build_app_menus;
pub use file_menu::{
    LoadEstimate, NewConnection, OpenConnection, OpenRecentConnection, SaveConnection,
    SaveConnectionAs, bind_menu_keys, build_menu_bar,
};
pub use form_rows::{
    SE_FIELD_WIDTH, SE_LABEL_WIDTH, make_display_row, make_header_row, make_input_row,
    make_input_row_fixed, make_labeled_row, make_labeled_row_fixed, make_select_row,
};
pub use inputs::{make_decimal_input, make_integer_input, make_text_input};
pub use preferences::{OpenPreferences, bind_preferences_keys, open_preferences};
pub use qbi_form::{QbiForm, QbiFormEvent};
pub use results_form::ResultForm;
pub use se_worksheet_form::SeWorksheetForm;
pub use theme::{apply_configured_theme, init_theme_colors, reapply_configured_theme};
pub use validation_view::{FieldVisibility, field_messages, form_error_banner, visible_issues};
pub use window::{AppWindow, ReloadConnection};
pub use window_geometry::{
    restore_saved_window_bounds, save_tracked_window_bounds, save_window_bounds, track_window,
    tracked_window,
};
pub use window_preferences::WindowPreferences;

pub(crate) use form_rows::{
    make_carryforward_display_row_with_help, make_display_row_with_help, make_help_slot,
    make_input_row_fixed_with_help, make_input_row_with_help,
};
pub(crate) use inputs::{set_decimal_input, set_input_value, set_optional_decimal_input};
