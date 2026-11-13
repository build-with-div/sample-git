//! The window layout: sidebar, toolbar, tabs and status bar.

use crate::actions;
use crate::ui::branches::branches_panel;
use crate::ui::changes::changes_panel;
use crate::ui::confirm::confirm_dialog;
use crate::ui::history::history_panel;
use crate::ui::sidebar::sidebar;
use crate::ui::style::colors;
use crate::ui::widgets::action_button;
use crate::{State, Tab};
use div::prelude::*;
use div_components::primitives::{TabsOptions, TabsState};
use div_components::{
    register_fonts, ButtonSize, ButtonVariant, Tabs, TabsContent, TabsContentProps, TabsList,
    TabsListProps, TabsProps, TabsTrigger, TabsTriggerProps,
};
use std::rc::Rc;

/// The whole window.
pub fn root(state: State) -> ViewNode {
    // Geist and Geist Mono; without them text falls back to system fonts.
    register_fonts(&current_document()).expect("the bundled fonts load");
    // A new selection loads its status. The work runs in a task so the
    // effect depends on the selection alone.
    create_effect(move || {
        let selected = state.selected.get();
        div::task::spawn_local(async move { actions::selection_changed(state, selected) }).detach();
    });
    let c = colors();
    // div-gap: 26. Layout time grows about 2.2 times with each nested flex
    // container, so the frame is absolutely positioned blocks: sidebar,
    // main area and status bar here, then the tabs and the Changes panes.
    // Flex is used only inside rows.
    view! {
        <div
            id="app"
            style:position="relative"
            style:height="100vh"
            style:font-family="Geist"
            style:font-size="14px"
            style:line-height="1.4285714"
            style:color={c.foreground}
            style:background-color={c.background}
        >
            {sidebar(state)}
            {main_area(state)}
            {status_bar(state)}
            {confirm_dialog(state)}
        </div>
    }
}

fn toolbar(state: State) -> ViewNode {
    let c = colors();
    let refresh = action_button(
        "Refresh",
        ButtonVariant::Outline,
        ButtonSize::Sm,
        Some("refresh".into()),
        move || state.selected.get().is_some(),
        move || actions::refresh_all(state),
    );
    view! {
        <div
            id="toolbar"
            style:box-sizing="border-box"
            style:display="flex"
            style:align-items="center"
            style:gap="12px"
            style:height="48px"
            style:flex-shrink="0"
            style:padding="0 16px"
            style:border-bottom-width="1"
            style:border-bottom-style="solid"
            style:border-bottom-color={c.border_line}
        >
            <div id="repo-title" style:font-weight="600" style:white-space="nowrap">
                {state
                    .selected_repo()
                    .map(|repo| repo.name)
                    .unwrap_or_else(|| "No repository selected".into())}
            </div>
            <div
                id="repo-summary"
                style:flex-grow="1"
                style:min-width="0"
                style:color={c.muted_foreground}
                style:overflow="hidden"
                style:text-overflow="ellipsis"
                style:white-space="nowrap"
            >
                {state
                    .selected
                    .get()
                    .map(|path| state.summary(&path).line())
                    .unwrap_or_default()}
            </div>
            {refresh}
        </div>
    }
}

fn main_area(state: State) -> ViewNode {
    // Controlled, as in Radix: the tabs report a selection and the app
    // writes it, so the View menu and the tabs share `state.tab`.
    let tabs = TabsState::new(TabsOptions {
        value: Some(state.tab),
        on_value_change: Some(Rc::new(move |value| state.tab.set(value))),
        ..TabsOptions::default()
    });
    let [t0, t1, t2] = Tab::ALL.map(|tab| {
        view! {
            <TabsTrigger state={tabs.clone()} value={tab.value()} disabled={false}>
                {tab.label()}
            </TabsTrigger>
        }
    });
    let [p0, p1, p2] = Tab::ALL.map(|tab| {
        let body = match tab {
            Tab::Changes => Show(
                move || state.selected.get().is_some(),
                move || placeholder(state, tab),
                move || changes_panel(state),
            ),
            Tab::History => Show(
                move || state.selected.get().is_some(),
                move || placeholder(state, tab),
                move || history_panel(state),
            ),
            Tab::Branches => Show(
                move || state.selected.get().is_some(),
                move || placeholder(state, tab),
                move || branches_panel(state),
            ),
        };
        let content = view! {
            <TabsContent state={tabs.clone()} value={tab.value()}>
                {body}
            </TabsContent>
        };
        // div-gap: 26. The panel fills the area below the tab list (12px
        // padding, a 36px list, an 8px gap) so its lists scroll inside it.
        if let ViewNode::Element(element) = &content {
            element
                .apply_style(
                    &style! { position: absolute; top: 56px; right: 0; bottom: 0; left: 0 },
                )
                .expect("style a tab panel");
        }
        content
    });
    let tabs_view = view! {
        <Tabs state={tabs.clone()}>
            <TabsList state={tabs.clone()}>
                {t0}
                {t1}
                {t2}
            </TabsList>
            {p0}
            {p1}
            {p2}
        </Tabs>
    };
    // div-gap: 26. The tabs root is a flex column; make it a block that
    // fills the area below the toolbar. (`TabsState::root` builds a new
    // element, so this styles the one `Tabs` returned.)
    if let ViewNode::Element(root) = &tabs_view {
        root.apply_style(&style! {
            display: block;
            position: absolute;
            top: 48px;
            right: 16px;
            bottom: 0;
            left: 16px;
            padding-top: 12px
        })
        .expect("style the tabs");
    }
    view! {
        <div
            id="main"
            style:position="absolute"
            style:top="0"
            style:right="0"
            style:bottom="24px"
            style:left="240px"
        >
            {toolbar(state)}
            {tabs_view}
        </div>
    }
}

/// What a tab shows until it has content for the selected repository.
fn placeholder(state: State, tab: Tab) -> ViewNode {
    let c = colors();
    view! {
        <div style:padding="24px 0" style:color={c.muted_foreground}>
            {match state.selected_repo() {
                Some(repo) => format!("{}: {}", tab.label(), repo.path.display()),
                None => "Select a repository in the list.".to_owned(),
            }}
        </div>
    }
}

fn status_bar(state: State) -> ViewNode {
    let c = colors();
    view! {
        <div
            id="status-bar"
            style:position="absolute"
            style:right="0"
            style:bottom="0"
            style:left="0"
            style:box-sizing="border-box"
            style:display="flex"
            style:align-items="center"
            style:height="24px"
            style:flex-shrink="0"
            style:padding="0 12px"
            style:font-size="12px"
            style:color={c.muted_foreground}
            style:background-color={c.muted}
            style:border-top-width="1"
            style:border-top-style="solid"
            style:border-top-color={c.border_line}
        >
            {state.status.get()}
        </div>
    }
}
