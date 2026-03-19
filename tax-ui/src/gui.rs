use anyhow::Result;
use gpui::{
    AnyElement, App, AppContext, ClickEvent, Context, InteractiveElement, IntoElement, KeyBinding,
    Menu, MenuItem, ParentElement, Styled, Window, px,
};
use gpui_component::{WindowExt, dialog::DialogButtonProps, h_flex, v_flex};
use tax_core::db::DbConfig;
use tracing::{info, warn};

#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::themes::apply_macos_system_theme;
use crate::{
    Quit,
    app::{self, se_tax_estimate},
    components::{ErrorDialog, EstimatedIncomeForm, SeWorksheetForm, make_button},
    models::EstimatedIncomeModel,
    quit,
};

pub fn setup_app(app_cx: &mut App) {
    gpui_component::init(app_cx);

    #[cfg(target_os = "macos")]
    apply_macos_system_theme(app_cx);
    #[cfg(target_os = "linux")]
    apply_linux_system_theme(app_cx);

    app_cx.activate(true);

    #[cfg(target_os = "macos")]
    app_cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

    #[cfg(not(target_os = "macos"))]
    app_cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("alt-F4", Quit, None),
    ]);

    app_cx.on_action(quit);

    app_cx.set_menus(vec![Menu {
        name: "Tax Estimator".into(),
        items: vec![MenuItem::action("Quit", Quit)],
    }]);
}

/// Builds the primary window content.
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
                            "calculate-estimates",
                            "Calculate SE Tax",
                            move |_click_event: &ClickEvent, window: &mut Window, cx: &mut App| {
                                let form_model = match form_handle.read(cx).to_model(cx) {
                                    Ok(m) => m,
                                    Err(errors) => {
                                        for e in &errors {
                                            warn!(%e, "form error");
                                        }
                                        ErrorDialog::show("Validation failed", &errors, window, cx);
                                        return;
                                    }
                                };
                                info!(%form_model, "Form validated\n");
                                cx.spawn(async move |_cx| {
                                    if let Err(e) = make_estimate(&form_model).await {
                                        warn!(%e, "Calculate SE Tax failed");
                                    }
                                })
                                .detach();
                            },
                        )
                    })
                    .child(make_button(
                        "open-se-worksheet",
                        "SE Worksheet",
                        move |_ev, window, cx| {
                            let worksheet = cx.new(|form_cx: &mut Context<SeWorksheetForm>| {
                                SeWorksheetForm::new(window, form_cx)
                            });
                            window.open_dialog(
                                cx,
                                move |dialog, _window: &mut Window, _cx: &mut App| {
                                    dialog
                                        .w(px(520.))
                                        .title("SE Tax Worksheet")
                                        .child(worksheet.clone())
                                        .button_props(
                                            DialogButtonProps::default().cancel_text("Close"),
                                        )
                                        .footer(|_ok, cancel, window, cx| vec![cancel(window, cx)])
                                },
                            );
                        },
                    )),
            )
            .into_any_element()
    }
}

async fn make_estimate(model: &EstimatedIncomeModel) -> Result<()> {
    let new_est = model.to_new_tax_estimate();
    se_tax_estimate(new_est, "taxes.db", "sqlite").await
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
