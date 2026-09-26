// file_menu.rs
use gpui::{Action, App, KeyBinding, ParentElement, Styled};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenu},
};

use super::edit_menu::build_edit_menu_button;
#[cfg(target_os = "macos")]
use super::edit_menu::edit_app_menu;
#[cfg(target_os = "macos")]
use super::preferences::{OpenPreferences, PREFERENCES_LABEL};
use super::recent_labels::recent_connection_labels;
use crate::Quit; // reuse the app-wide action
use crate::config::{AppConfig, RecentConnection};

/// Label of the submenu that lists the recent connections.
const RECENT_MENU_LABEL: &str = "Recent...";

/// Label of the disabled row shown while the recent list is empty.
const NO_RECENT_LABEL: &str = "No Recent Connections";

// Add any new actions you need
gpui::actions!(
    tax_estimator,
    [
        NewConnection,
        OpenConnection,
        SaveConnection,
        SaveConnectionAs,
        LoadEstimate,
        NoRecentConnections
    ]
);

/// Switches to one connection from the recent list.
///
/// The action carries the connection itself, so it stays correct even if the
/// list changes between building the menu and choosing the row.
#[derive(Clone, PartialEq, Action)]
#[action(namespace = tax_estimator, no_json)]
pub struct OpenRecentConnection {
    pub connection: RecentConnection,
}

pub fn bind_menu_keys(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewConnection, None),
        KeyBinding::new("cmd-o", OpenConnection, None),
        KeyBinding::new("cmd-s", SaveConnection, None),
        KeyBinding::new("cmd-shift-s", SaveConnectionAs, None),
        KeyBinding::new("cmd-l", LoadEstimate, None),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-n", NewConnection, None),
        KeyBinding::new("ctrl-o", OpenConnection, None),
        KeyBinding::new("ctrl-s", SaveConnection, None),
        KeyBinding::new("ctrl-shift-s", SaveConnectionAs, None),
        KeyBinding::new("ctrl-l", LoadEstimate, None),
    ]);
}

/// The recent connections paired with their menu labels, most recent first.
fn recent_entries(cx: &App) -> Vec<(String, RecentConnection)> {
    let connections = &AppConfig::get(cx).recent.connections;
    recent_connection_labels(connections)
        .into_iter()
        .zip(connections.iter().cloned())
        .collect()
}

/// Fills the in-window *Recent...* submenu: one row per entry, or a single
/// disabled row when there are none.
fn add_recent_items(
    menu: PopupMenu,
    entries: &[(String, RecentConnection)],
) -> PopupMenu {
    if entries.is_empty() {
        return menu.menu_with_disabled(NO_RECENT_LABEL, Box::new(NoRecentConnections), true);
    }

    entries.iter().fold(menu, |menu, (label, connection)| {
        let action = OpenRecentConnection {
            connection: connection.clone(),
        };
        menu.menu(label.clone(), Box::new(action))
    })
}

/// Builds an in-window menu bar for Linux/Windows.
///
/// The dropdown is built each time it opens, so the *Recent...* submenu
/// always shows the current list.
pub fn build_menu_bar() -> impl gpui::IntoElement {
    h_flex()
        .gap_0()
        .child(
            Button::new("file-menu")
                .label("File")
                .ghost()
                .xsmall()
                .dropdown_menu(|menu, window, cx| {
                    let entries = recent_entries(cx);

                    menu.menu_with_icon("New Connection", IconName::File, Box::new(NewConnection))
                        .menu_with_icon(
                            "Open Connection",
                            IconName::FolderOpen,
                            Box::new(OpenConnection),
                        )
                        .menu("Load Estimate", Box::new(LoadEstimate))
                        .separator()
                        .submenu(RECENT_MENU_LABEL, window, cx, move |submenu, _, _| {
                            add_recent_items(submenu, &entries)
                        })
                        .separator()
                        .menu("Save", Box::new(SaveConnection))
                        .menu("Save As...", Box::new(SaveConnectionAs))
                        .separator()
                        .menu("Quit", Box::new(Quit))
                }),
        )
        .child(build_edit_menu_button())
}

/// The native menu row that reopens `connection`.
#[cfg(target_os = "macos")]
fn recent_menu_item((label, connection): (String, RecentConnection)) -> MenuItem {
    MenuItem::action(label, OpenRecentConnection { connection })
}

/// Builds the native macOS menu bar from the current configuration.
///
/// Nothing handles [`NoRecentConnections`], so macOS shows that row disabled.
#[cfg(target_os = "macos")]
pub fn build_app_menus(cx: &App) -> Vec<Menu> {
    let entries = recent_entries(cx);
    let recent_items = if entries.is_empty() {
        vec![MenuItem::action(NO_RECENT_LABEL, NoRecentConnections)]
    } else {
        entries.into_iter().map(recent_menu_item).collect()
    };

    vec![
        Menu {
            name: "Tax Estimator".into(),
            items: vec![
                MenuItem::action(PREFERENCES_LABEL, OpenPreferences),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Connection", NewConnection),
                MenuItem::action("Open Connection", OpenConnection),
                MenuItem::action("Load Estimate", LoadEstimate),
                MenuItem::separator(),
                MenuItem::submenu(Menu {
                    name: RECENT_MENU_LABEL.into(),
                    items: recent_items,
                }),
                MenuItem::separator(),
                MenuItem::action("Save", SaveConnection),
                MenuItem::action("Save As...", SaveConnectionAs),
            ],
        },
        edit_app_menu(),
    ]
}
