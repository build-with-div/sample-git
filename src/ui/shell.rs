//! The window layout: sidebar, toolbar, tabs and status bar.

use crate::actions;
use crate::ui::sidebar::sidebar;
use crate::ui::style::{colors, Slot, SlotProps};
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
    let c = colors();
    view! {
        <div
            id="app"
            style:display="flex"
            style:flex-direction="column"
            style:height="100vh"
            style:font-family="Geist"
            style:font-size="14px"
            style:line-height="1.4285714"
            style:color={c.foreground}
            style:background-color={c.background}
        >
            <div style:display="flex" style:flex-grow="1" style:min-height="0">
                <Slot view={sidebar(state)} />
                <Slot view={main_area(state)} />
            </div>
            <Slot view={status_bar(state)} />
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
            <Slot view={refresh} />
        </div>
    }
}

fn main_area(state: State) -> ViewNode {
    let c = colors();
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
        view! {
            <TabsContent state={tabs.clone()} value={tab.value()}>
                <div style:padding="24px 0" style:color={c.muted_foreground}>
                    {match state.selected_repo() {
                        Some(repo) => format!("{}: {}", tab.label(), repo.path.display()),
                        None => "Select a repository in the list.".to_owned(),
                    }}
                </div>
            </TabsContent>
        }
    });
    view! {
        <div
            id="main"
            style:display="flex"
            style:flex-direction="column"
            style:flex-grow="1"
            style:min-width="0"
        >
            <Slot view={toolbar(state)} />
            <div style:padding="12px 16px 0 16px" style:flex-grow="1" style:min-height="0">
                <Tabs state={tabs.clone()}>
                    <TabsList state={tabs.clone()}>
                        <Slot view={t0} />
                        <Slot view={t1} />
                        <Slot view={t2} />
                    </TabsList>
                    <Slot view={p0} />
                    <Slot view={p1} />
                    <Slot view={p2} />
                </Tabs>
            </div>
        </div>
    }
}

fn status_bar(state: State) -> ViewNode {
    let c = colors();
    view! {
        <div
            id="status-bar"
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
