//! The menu bar: native on Windows, drawn by Div on Linux.

use crate::{State, Tab};
use div::menu::{Accelerator, Key, Menu, MenuItem, Modifiers, Role};

pub fn app_menu(state: State) -> Menu {
    Menu::new()
        .submenu("&File", |menu| menu.role(Role::Quit))
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
