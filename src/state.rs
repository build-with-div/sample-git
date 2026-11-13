//! App state shared by the views and the menu. Signals are cheap handles,
//! so the struct is `Copy`.

use div::prelude::*;

/// The tabs of the main area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Changes,
    History,
    Branches,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Changes, Tab::History, Tab::Branches];

    /// The value the tabs component uses for this tab.
    pub fn value(self) -> &'static str {
        match self {
            Tab::Changes => "changes",
            Tab::History => "history",
            Tab::Branches => "branches",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Changes => "Changes",
            Tab::History => "History",
            Tab::Branches => "Branches",
        }
    }
}

#[derive(Clone, Copy)]
pub struct State {
    /// The selected tab, as its [`Tab::value`].
    pub tab: Signal<String>,
    /// What the last operation reported, shown in the status bar.
    pub status: Signal<String>,
}

impl State {
    pub fn new() -> Self {
        Self {
            tab: create_signal(Tab::Changes.value().to_owned()),
            status: create_signal("Ready".to_owned()),
        }
    }

    pub fn select_tab(&self, tab: Tab) {
        self.tab.set(tab.value().to_owned());
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
