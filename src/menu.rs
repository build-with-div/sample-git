//! The menu bar: native on Windows, drawn by Div on Linux.

use crate::{actions, State, Tab};
use div::menu::{Accelerator, Key, Menu, MenuItem, Modifiers, Role};

pub fn app_menu(state: State) -> Menu {
    Menu::new()
        .submenu("&File", |menu| {
            menu.item(
                MenuItem::new("&Add Repository...")
                    .accelerator(Accelerator::new(Modifiers::PRIMARY, Key::O))
                    .on_select(move || actions::pick_and_add(state)),
            )
            .item(MenuItem::new("&Scan Folder...").on_select(move || actions::pick_and_scan(state)))
            .item(
                MenuItem::new("&Remove from List")
                    .enabled(move || state.selected.get().is_some())
                    .on_select(move || {
                        if let Some(path) = state.selected.get() {
                            actions::remove_repo(state, &path);
                        }
                    }),
            )
            .separator()
            .role(Role::Quit)
        })
        .submenu("&Repository", |menu| {
            menu.item(
                MenuItem::new("&Refresh")
                    .accelerator(Accelerator::new(Modifiers::NONE, Key::Function(5)))
                    .on_select(move || actions::refresh_all(state)),
            )
        })
        .submenu("&View", |menu| {
            let keys = [Key::Char('1'), Key::Char('2'), Key::Char('3')];
            Tab::ALL
                .into_iter()
                .zip(keys)
                .fold(menu, |menu, (tab, key)| {
                    menu.item(
                        MenuItem::new(tab.label())
                            .accelerator(Accelerator::new(Modifiers::PRIMARY, key))
                            .on_select(move || state.select_tab(tab)),
                    )
                })
        })
        .submenu("&Help", |menu| menu.role(Role::About))
}
