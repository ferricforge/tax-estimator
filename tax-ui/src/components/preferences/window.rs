//! The view inside the preferences window: the form, its footer buttons, and
//! the commit logic.
//!
//! On macOS each change is saved as it is made. On Windows and Linux, changes
//! are saved when Apply or OK is chosen, and Cancel discards them. Appearance
//! changes follow the same rule: the theme is applied when the changes are
//! saved.

use gpui::{
    App, AppContext, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, PromptLevel, Render, StatefulInteractiveElement as _, Styled, Subscription,
    Window, div, px,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    switch::Switch,
    v_flex,
};

use crate::components::{
    ReloadConnection, reapply_configured_theme, save_window_bounds, tracked_window,
};
use crate::config::{
    AppConfig, FieldError, MAIN_WINDOW, PREFERENCES_WINDOW, PreferenceField, PreferencesDraft,
};
use crate::logging::{SourcePathDisplay, apply_settings};
use crate::themes::ThemeMode;

/// Width of the label column in each row, in logical pixels.
const LABEL_WIDTH: f32 = 200.0;

/// Title of the question asked after pool settings change.
const RELOAD_TITLE: &str = "Reload connection?";

/// Explanation shown with the reload question.
const RELOAD_DETAIL: &str = "Pool settings take effect when the database connection is \
    reopened. Estimates are saved as they are calculated, so reloading does not lose data.";

/// Answers offered with the reload question. The first one reloads.
const RELOAD_ANSWERS: [&str; 2] = ["Reload Now", "Later"];

/// Source path choices, in the order the cycle button steps through them.
const SOURCE_PATH_CHOICES: [SourcePathDisplay; 4] = [
    SourcePathDisplay::Full,
    SourcePathDisplay::Short,
    SourcePathDisplay::FileName,
    SourcePathDisplay::Hidden,
];

/// The label shown on a theme button.
fn theme_mode_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "System",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    }
}

/// The element id of a theme button.
fn theme_mode_id(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "theme-system",
        ThemeMode::Light => "theme-light",
        ThemeMode::Dark => "theme-dark",
    }
}

/// The name shown on the source path button.
fn source_path_name(path: SourcePathDisplay) -> &'static str {
    match path {
        SourcePathDisplay::Full => "full",
        SourcePathDisplay::Short => "short",
        SourcePathDisplay::FileName => "file_name",
        SourcePathDisplay::Hidden => "hidden",
    }
}

/// The choice after `current` in `choices`, wrapping to the first choice.
fn next_choice<T: Copy + PartialEq, const N: usize>(
    choices: [T; N],
    current: T,
) -> T {
    let index = choices
        .iter()
        .position(|choice| *choice == current)
        .unwrap_or(0);
    choices[(index + 1) % N]
}

/// A text input holding `value`.
fn text_input<T: 'static>(
    value: &str,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Entity<InputState> {
    let input = cx.new(|input_cx| InputState::new(window, input_cx));
    input.update(cx, |state, state_cx| {
        state.set_value(value.to_string(), window, state_cx)
    });
    input
}

/// A row with a label and a control.
fn labeled_control(
    label: &str,
    control: impl IntoElement,
) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(div().w(px(LABEL_WIDTH)).child(label.to_string()))
        .child(control)
}

/// A heading for one group of rows.
fn section_title(title: &str) -> impl IntoElement {
    div().pt_2().child(title.to_string())
}

/// Asks, in the main window, whether to reload the connection so that changed
/// pool settings take effect. Choosing *Reload Now* sends
/// [`ReloadConnection`] to the main window.
fn offer_pool_reload(cx: &mut App) {
    cx.spawn(async move |async_cx| {
        let Ok(Some(main_window)) =
            async_cx.update(|app_cx: &mut App| tracked_window(MAIN_WINDOW, app_cx))
        else {
            tracing::warn!("no main window to ask about reloading the connection");
            return;
        };

        let answer = main_window.update(async_cx, |_, window, cx| {
            window.prompt(
                PromptLevel::Info,
                RELOAD_TITLE,
                Some(RELOAD_DETAIL),
                &RELOAD_ANSWERS,
                cx,
            )
        });
        let Ok(answer) = answer else {
            return;
        };

        if let Ok(0) = answer.await {
            let _ = main_window.update(async_cx, |_, window, cx| {
                window.dispatch_action(Box::new(ReloadConnection), cx);
            });
        }
    })
    .detach();
}

pub struct PreferencesWindow {
    level: Entity<InputState>,
    file_path: Entity<InputState>,
    max_connections: Entity<InputState>,
    min_connections: Entity<InputState>,
    acquire_timeout_secs: Entity<InputState>,
    idle_timeout_secs: Entity<InputState>,
    max_lifetime_secs: Entity<InputState>,
    recent_limit: Entity<InputState>,
    theme: ThemeMode,
    application_only: bool,
    stdout: bool,
    file_enabled: bool,
    source_path: SourcePathDisplay,
    test_before_acquire: bool,
    errors: Vec<FieldError>,
    _subscriptions: Vec<Subscription>,
}

impl PreferencesWindow {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.on_window_should_close(cx, |window, app_cx| {
            save_window_bounds(PREFERENCES_WINDOW, window, app_cx);
            true
        });

        let draft = PreferencesDraft::from_config(AppConfig::get(cx));

        let level = text_input(&draft.level, window, cx);
        let file_path = text_input(&draft.file_path, window, cx);
        let max_connections = text_input(&draft.max_connections, window, cx);
        let min_connections = text_input(&draft.min_connections, window, cx);
        let acquire_timeout_secs = text_input(&draft.acquire_timeout_secs, window, cx);
        let idle_timeout_secs = text_input(&draft.idle_timeout_secs, window, cx);
        let max_lifetime_secs = text_input(&draft.max_lifetime_secs, window, cx);
        let recent_limit = text_input(&draft.recent_limit, window, cx);

        let inputs = [
            &level,
            &file_path,
            &max_connections,
            &min_connections,
            &acquire_timeout_secs,
            &idle_timeout_secs,
            &max_lifetime_secs,
            &recent_limit,
        ];
        let subscriptions: Vec<Subscription> = inputs
            .into_iter()
            .map(|input| Self::changed_on_leave(input, window, cx))
            .collect();

        Self {
            level,
            file_path,
            max_connections,
            min_connections,
            acquire_timeout_secs,
            idle_timeout_secs,
            max_lifetime_secs,
            recent_limit,
            theme: draft.theme,
            application_only: draft.application_only,
            stdout: draft.stdout,
            file_enabled: draft.file_enabled,
            source_path: draft.source_path,
            test_before_acquire: draft.test_before_acquire,
            errors: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Reports a change when a text field loses focus or Enter is pressed.
    fn changed_on_leave(
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            input,
            window,
            |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.changed(cx);
                }
            },
        )
    }

    /// Called after a control changes. macOS saves at once; other platforms
    /// wait for Apply or OK.
    fn changed(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if cfg!(target_os = "macos") {
            self.commit(cx);
        } else {
            cx.notify();
        }
    }

    /// The form's current values.
    fn draft(
        &self,
        cx: &App,
    ) -> PreferencesDraft {
        PreferencesDraft {
            theme: self.theme,
            level: self.level.read(cx).value().to_string(),
            application_only: self.application_only,
            stdout: self.stdout,
            file_enabled: self.file_enabled,
            file_path: self.file_path.read(cx).value().to_string(),
            source_path: self.source_path,
            max_connections: self.max_connections.read(cx).value().to_string(),
            min_connections: self.min_connections.read(cx).value().to_string(),
            acquire_timeout_secs: self.acquire_timeout_secs.read(cx).value().to_string(),
            idle_timeout_secs: self.idle_timeout_secs.read(cx).value().to_string(),
            max_lifetime_secs: self.max_lifetime_secs.read(cx).value().to_string(),
            test_before_acquire: self.test_before_acquire,
            recent_limit: self.recent_limit.read(cx).value().to_string(),
        }
    }

    /// Validates the form and, when it is valid, saves and applies the
    /// preferences. Returns whether they were saved.
    ///
    /// The theme is applied from a spawned task, after this update finishes.
    /// When the pool settings changed, the user is also asked whether to
    /// reload the connection.
    fn commit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let preferences = match self.draft(cx).validate() {
            Ok(preferences) => preferences,
            Err(errors) => {
                self.errors = errors;
                cx.notify();
                return false;
            }
        };
        self.errors.clear();

        let pool_changed = AppConfig::get(cx).database.pool != preferences.pool;
        AppConfig::update(cx, |config| preferences.apply_to(config));

        if let Err(error) = AppConfig::save(cx) {
            tracing::error!(%error, "failed to save preferences");
        }
        if let Err(error) = apply_settings(&AppConfig::get(cx).logging.settings()) {
            tracing::error!(%error, "failed to apply logging settings");
        }

        reapply_configured_theme(cx);

        if pool_changed {
            offer_pool_reload(cx);
        }
        cx.notify();
        true
    }

    /// A labeled text field, with the message for `field` when it is invalid.
    fn field_row(
        &self,
        label: &str,
        field: PreferenceField,
        input: &Entity<InputState>,
    ) -> impl IntoElement {
        let message = self
            .errors
            .iter()
            .find(|error| error.field == field)
            .map(|error| error.message.clone());

        v_flex()
            .gap_1()
            .child(labeled_control(label, Input::new(input)))
            .children(message.map(|message| div().text_size(px(11.0)).child(message)))
    }

    /// One button per theme choice. The active choice is filled.
    fn theme_button(
        &self,
        mode: ThemeMode,
        cx: &Context<Self>,
    ) -> Button {
        let button = Button::new(theme_mode_id(mode))
            .label(theme_mode_name(mode))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                if this.theme != mode {
                    this.theme = mode;
                    this.changed(cx);
                }
            }));

        if mode == self.theme {
            button.primary()
        } else {
            button
        }
    }

    /// Turns every hidden confirmation back on and saves the change.
    ///
    /// This takes effect at once on every platform, because it is an action
    /// rather than an edited field.
    fn reset_dialogs(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let restored = AppConfig::update(cx, |config| config.dialogs.reset());
        if restored == 0 {
            return;
        }

        if let Err(error) = AppConfig::save(cx) {
            tracing::error!(%error, "failed to save the hidden confirmations");
        }
        cx.notify();
    }

    /// How many confirmations are hidden, and a button that shows them again.
    fn render_dialogs(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let hidden = AppConfig::get(cx).dialogs.suppressed_count();
        let summary = match hidden {
            0 => "No confirmations are hidden.".to_string(),
            1 => "1 confirmation is hidden.".to_string(),
            count => format!("{count} confirmations are hidden."),
        };
        let reset = Button::new("reset-dialogs")
            .label("Show all again")
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.reset_dialogs(cx);
            }));

        v_flex()
            .gap_2()
            .child(section_title("Confirmations"))
            .child(labeled_control("Hidden confirmations", reset))
            .child(div().text_size(px(11.0)).child(summary))
    }

    fn render_appearance(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let choices = ThemeMode::ALL.map(|mode| self.theme_button(mode, cx));

        v_flex()
            .gap_2()
            .child(section_title("Appearance"))
            .child(labeled_control("Theme", h_flex().gap_1().children(choices)))
    }

    fn render_logging(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let source_path = Button::new("source-path")
            .label(source_path_name(self.source_path))
            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.source_path = next_choice(SOURCE_PATH_CHOICES, this.source_path);
                this.changed(cx);
            }));

        let application_only = Switch::new("application-only")
            .checked(self.application_only)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.application_only = *checked;
                this.changed(cx);
            }));

        let stdout = Switch::new("stdout")
            .checked(self.stdout)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.stdout = *checked;
                this.changed(cx);
            }));

        let file_enabled = Switch::new("file-enabled")
            .checked(self.file_enabled)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.file_enabled = *checked;
                this.changed(cx);
            }));

        v_flex()
            .gap_2()
            .child(section_title("Logging"))
            .child(self.field_row("Level", PreferenceField::Level, &self.level))
            .child(labeled_control("Source path", source_path))
            .child(labeled_control("Only this application", application_only))
            .child(labeled_control("Write to standard output", stdout))
            .child(labeled_control("Write to a file", file_enabled))
            .child(self.field_row("File path", PreferenceField::FilePath, &self.file_path))
    }

    fn render_pool(
        &self,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let test_before_acquire = Switch::new("test-before-acquire")
            .checked(self.test_before_acquire)
            .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                this.test_before_acquire = *checked;
                this.changed(cx);
            }));

        v_flex()
            .gap_2()
            .child(section_title("Database connection pool"))
            .child(self.field_row(
                "Maximum connections",
                PreferenceField::MaxConnections,
                &self.max_connections,
            ))
            .child(self.field_row(
                "Minimum connections",
                PreferenceField::MinConnections,
                &self.min_connections,
            ))
            .child(self.field_row(
                "Acquire timeout (seconds)",
                PreferenceField::AcquireTimeout,
                &self.acquire_timeout_secs,
            ))
            .child(self.field_row(
                "Idle timeout (seconds, 0 for none)",
                PreferenceField::IdleTimeout,
                &self.idle_timeout_secs,
            ))
            .child(self.field_row(
                "Maximum lifetime (seconds, 0 for none)",
                PreferenceField::MaxLifetime,
                &self.max_lifetime_secs,
            ))
            .child(labeled_control("Test before use", test_before_acquire))
            .child(
                div().text_size(px(11.0)).child(
                    "Pool changes take effect the next time a connection opens.".to_string(),
                ),
            )
    }

    fn render_recent(&self) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(section_title("Recent connections"))
            .child(self.field_row(
                "Connections to remember",
                PreferenceField::RecentLimit,
                &self.recent_limit,
            ))
    }

    /// The Cancel, Apply, and OK buttons. macOS has none: it saves as changes
    /// are made.
    fn render_footer(
        &self,
        cx: &Context<Self>,
    ) -> Option<impl IntoElement> {
        if cfg!(target_os = "macos") {
            return None;
        }

        let cancel = cx.listener(|_this, _: &ClickEvent, window, _cx| {
            window.remove_window();
        });
        let apply = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.commit(cx);
        });
        let ok = cx.listener(|this, _: &ClickEvent, window, cx| {
            if this.commit(cx) {
                window.remove_window();
            }
        });

        Some(
            h_flex()
                .p_3()
                .gap_2()
                .justify_end()
                .child(
                    Button::new("preferences-cancel")
                        .label("Cancel")
                        .on_click(cancel),
                )
                .child(
                    Button::new("preferences-apply")
                        .label("Apply")
                        .on_click(apply),
                )
                .child(
                    Button::new("preferences-ok")
                        .label("OK")
                        .primary()
                        .on_click(ok),
                ),
        )
    }
}

impl Render for PreferencesWindow {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .id("preferences-body")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .p_5()
                    .gap_4()
                    .child(self.render_appearance(cx))
                    .child(self.render_logging(cx))
                    .child(self.render_pool(cx))
                    .child(self.render_recent())
                    .child(self.render_dialogs(cx)),
            )
            .children(self.render_footer(cx))
    }
}
