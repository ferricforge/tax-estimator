use anyhow::Result;
use gpui::KeyBinding;
use gpui::{
    Action, AnyElement, App, AppContext, ClickEvent, Context, Entity, InteractiveElement,
    IntoElement, ParentElement, Styled, Window, px,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use gpui_component::{WindowExt, dialog::DialogButtonProps, h_flex, v_flex};
use tracing::{info, warn};

#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::themes::apply_macos_system_theme;
#[cfg(target_os = "windows")]
use crate::themes::apply_windows_system_theme;
use crate::{
    Quit,
    app::se_tax_estimate,
    components::{
        CloseProject, ErrorDialog, EstimatedIncomeForm, NewProject, OpenProject, SaveProject,
        SaveProjectAs, SeWorksheetForm, bind_menu_keys, init_theme_colors, make_button,
    },
    models::EstimatedIncomeModel,
    quit,
};

/// Registers a handler for a GPUI [`Action`] type.
fn register_action<A: Action>(
    app: &mut App,
    f: impl Fn(&A, &mut App) + 'static,
) {
    app.on_action(f);
}

fn stub_file_action<A: Action>(name: &'static str) -> impl Fn(&A, &mut App) {
    move |_, _| {
        tracing::info!("{name}: not yet implemented");
    }
}

pub fn setup_app(app_cx: &mut App) {
    gpui_component::init(app_cx);

    #[cfg(target_os = "macos")]
    apply_macos_system_theme(app_cx);
    #[cfg(target_os = "windows")]
    apply_windows_system_theme(app_cx);
    #[cfg(target_os = "linux")]
    apply_linux_system_theme(app_cx);

    // Populate legacy theme constants from the now-active theme.
    init_theme_colors(app_cx);

    #[cfg(target_os = "macos")]
    app_cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

    #[cfg(not(target_os = "macos"))]
    app_cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("alt-F4", Quit, None),
    ]);

    app_cx.on_action(quit);

    register_action(app_cx, stub_file_action::<NewProject>("NewProject"));
    register_action(app_cx, stub_file_action::<OpenProject>("OpenProject"));
    register_action(app_cx, stub_file_action::<SaveProject>("SaveProject"));
    register_action(app_cx, stub_file_action::<SaveProjectAs>("SaveProjectAs"));
    register_action(app_cx, stub_file_action::<CloseProject>("CloseProject"));

    bind_menu_keys(app_cx);

    // Native macOS menu bar
    #[cfg(target_os = "macos")]
    app_cx.set_menus(vec![
        Menu {
            name: "Tax Estimator".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Project", NewProject),
                MenuItem::action("Open Project", OpenProject),
                MenuItem::separator(),
                MenuItem::action("Save", SaveProject),
                MenuItem::action("Save As...", SaveProjectAs),
                MenuItem::separator(),
                MenuItem::action("Close Project", CloseProject),
            ],
        },
    ]);
    app_cx.activate(true);
}

/// Builds the primary window content.
pub fn build_main_content(
    window: &mut Window,
    app_cx: &mut App,
) -> impl Fn() -> AnyElement + 'static {
    let form = app_cx.new(|form_cx: &mut Context<EstimatedIncomeForm>| {
        EstimatedIncomeForm::new(window, form_cx)
    });

    let worksheet =
        app_cx.new(|form_cx: &mut Context<SeWorksheetForm>| SeWorksheetForm::new(window, form_cx));

    move || {
        let root = {
            let base = v_flex().size_full().gap_0(); // gap_0 so menu bar sits flush
            #[cfg(not(target_os = "macos"))]
            {
                base.child(build_menu_bar())
            }
            #[cfg(target_os = "macos")]
            {
                base
            }
        };

        root.child(main_body(form.clone(), worksheet.clone()))
            .into_any_element()
    }
}

fn main_body(
    form: Entity<EstimatedIncomeForm>,
    worksheet: Entity<SeWorksheetForm>,
) -> impl IntoElement {
    v_flex()
        .size_full()
        .p_5()
        .gap_4()
        .child(form.clone())
        .child(main_toolbar(form, worksheet))
}

fn main_toolbar(
    form: Entity<EstimatedIncomeForm>,
    worksheet: Entity<SeWorksheetForm>,
) -> impl IntoElement {
    let form_calc = form.clone();
    h_flex()
        .id("window-body")
        .p_1()
        .gap_4()
        .items_center()
        .justify_center()
        .child(make_button(
            "calculate-estimates",
            "Calculate SE Tax",
            move |_click_event: &ClickEvent, window: &mut Window, cx: &mut App| {
                spawn_calculate_se_tax(&form_calc, window, cx);
            },
        ))
        .child(make_button(
            "open-se-worksheet",
            "SE Worksheet",
            move |_ev, window, cx| {
                open_se_worksheet_dialog(worksheet.clone(), window, cx);
            },
        ))
}

fn model_from_form_or_show_errors(
    form: &Entity<EstimatedIncomeForm>,
    window: &mut Window,
    cx: &mut App,
) -> Option<EstimatedIncomeModel> {
    match form.read(cx).to_model(cx) {
        Ok(m) => Some(m),
        Err(errors) => {
            for e in &errors {
                warn!(%e, "form error");
            }
            ErrorDialog::show("Validation failed", &errors, window, cx);
            None
        }
    }
}

fn spawn_calculate_se_tax(
    form: &Entity<EstimatedIncomeForm>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(form_model) = model_from_form_or_show_errors(form, window, cx) else {
        return;
    };
    info!(%form_model, "Form validated\n");
    cx.spawn(async move |_cx| {
        if let Err(e) = make_estimate(&form_model).await {
            warn!(%e, "Calculate SE Tax failed");
        }
    })
    .detach();
}

fn open_se_worksheet_dialog(
    worksheet: Entity<SeWorksheetForm>,
    window: &mut Window,
    cx: &mut App,
) {
    let worksheet_for_dialog = worksheet.clone();
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .overlay_closable(false)
            .w(px(520.0))
            .margin_top(px(-20.0))
            .title("SE Tax Worksheet")
            .child(worksheet_for_dialog.clone())
            .button_props(DialogButtonProps::default().cancel_text("Close"))
            .footer(|_ok, cancel, window, cx| vec![cancel(window, cx)])
    });
}

async fn make_estimate(model: &EstimatedIncomeModel) -> Result<()> {
    let new_est = model.to_new_tax_estimate();
    se_tax_estimate(new_est, "taxes.db", "sqlite").await
}
