use anyhow::Result;
use gpui::{
    AnyElement, App, AppContext, Context, InteractiveElement, IntoElement, KeyBinding, Menu,
    MenuItem, ParentElement, Styled, Window,
};
use gpui_component::{h_flex, v_flex};
use tax_core::db::DbConfig;
use tracing::{info, warn};

#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::themes::apply_macos_system_theme;
use crate::{
    Quit, app,
    components::{EstimatedIncomeForm, make_button},
    models::EstimatedIncomeModel,
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
                        make_button("load-data", "Load Data", move |_, _, cx: &mut App| {
                            let form_model = match form_handle.read(cx).to_model(cx) {
                                Ok(m) => m,
                                Err(errors) => {
                                    for e in &errors {
                                        warn!(%e, "form error");
                                    }
                                    return;
                                }
                            };
                            let year = form_model.tax_year;
                            cx.spawn(async move |_cx| {
                                if let Err(e) = load_some_data("taxes.db", "sqlite", year).await {
                                    warn!(%e, "Load Data failed");
                                }
                            })
                            .detach();
                        })
                    })
                    .child({
                        let form_handle = form.clone();
                        make_button(
                            "convert-files",
                            "Convert Files",
                            move |_, _, cx: &mut App| {
                                let form_model = match form_handle.read(cx).to_model(cx) {
                                    Ok(m) => m,
                                    Err(errors) => {
                                        for e in &errors {
                                            warn!(%e, "form error");
                                        }
                                        return;
                                    }
                                };
                                info!(%form_model, "Form validated\n");
                                make_estimate(&form_model);
                            },
                        )
                    }),
            )
            .into_any_element()
    }
}

fn make_estimate(model: &EstimatedIncomeModel) {
    let new_est = model.to_new_tax_estimate();
    info!(%new_est, "New Estimate");
}

async fn load_some_data(
    db_connection: &str,
    backend: &str,
    year: i32,
) -> Result<()> {
    let db_config = DbConfig {
        backend: backend.to_string(),
        connection_string: db_connection.to_string(),
    };

    tracing::debug!("connecting to {} backend", db_config.backend);
    let registry = app::build_registry();
    let repo = registry.create(&db_config).await?;

    let data = app::load_tax_year_data(&*repo, year).await?;
    info!("{}", data);
    Ok(())
}
