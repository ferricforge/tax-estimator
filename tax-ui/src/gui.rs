use gpui::{
    AnyElement, App, AppContext, Context, InteractiveElement, IntoElement, KeyBinding, Menu,
    MenuItem, ParentElement, Styled, Window,
};
use gpui_component::{h_flex, v_flex};
use tracing::{info, warn};

#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::{Quit, themes::apply_macos_system_theme};
use crate::{
    components::{EstimatedIncomeForm, make_button},
    quit,
};

pub fn setup_app(app_cx: &mut App) {
    // This must be called before using any GPUI Component features.
    gpui_component::init(app_cx);

    #[cfg(target_os = "macos")]
    apply_macos_system_theme(app_cx);
    #[cfg(target_os = "linux")]
    apply_linux_system_theme(app_cx);

    app_cx.activate(true);

    // Bind platform-appropriate quit shortcut
    #[cfg(target_os = "macos")]
    app_cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

    #[cfg(not(target_os = "macos"))]
    app_cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("alt-F4", Quit, None),
    ]);

    // Register the quit action handler
    app_cx.on_action(quit);

    // Set up the application menu with Quit
    app_cx.set_menus(vec![Menu {
        name: "Tax Estimator".into(),
        items: vec![MenuItem::action("Quit", Quit)],
    }]);
}

/// Builds the primary window content.
///
/// Returns a closure suitable for passing to `Window::set_content`,
/// producing a styled "Click Me!" button on each render frame.
pub fn build_main_content(
    window: &mut Window,
    app_cx: &mut App,
) -> impl Fn() -> AnyElement + 'static {
    let form = app_cx.new(|form_cx: &mut Context<EstimatedIncomeForm>| {
        EstimatedIncomeForm::new(window, form_cx)
    });

    move || {
        v_flex()
            .size_full()
            .p_5()
            .gap_4()
            .child(form.clone())
            .child(
                h_flex()
                    .id("window-body")
                    .p_1()
                    .gap_4()
                    .items_center()
                    .justify_center()
                    .child({
                        let form_handle = form.clone();
                        make_button("ok-go", "Convert Files", move |_, _, cx: &mut App| {
                            let form_model = match form_handle.read(cx).to_model(cx) {
                                Ok(m) => m,
                                Err(e) => {
                                    warn!(%e, "Invalid decimal in form");
                                    return;
                                }
                            };
                            match form_model.validate_for_submit() {
                                Ok(()) => {
                                    info!(%form_model, "Form validated\n");
                                    // Next step: pass validated model to the processing crate.
                                }
                                Err(errors) => {
                                    warn!("Cannot submit form due to validation errors");
                                    for error in errors {
                                        warn!(%error, "validation error");
                                    }
                                }
                            }
                        })
                    }),
            )
            .into_any_element()
    }
}
