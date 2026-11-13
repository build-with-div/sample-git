//! The window layout: sidebar, toolbar, tabs and status bar.

use crate::ui::style::{colors, Slot, SlotProps};
use crate::{State, Tab};
use div::prelude::*;
use div_components::primitives::{TabsOptions, TabsState};
use div_components::{
    register_fonts, Button, ButtonProps, ButtonSize, ButtonVariant, Tabs, TabsContent,
    TabsContentProps, TabsList, TabsListProps, TabsProps, TabsTrigger, TabsTriggerProps,
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
                <Slot view={sidebar()} />
                <Slot view={main_area(state)} />
            </div>
            <Slot view={status_bar(state)} />
        </div>
    }
}

fn sidebar() -> ViewNode {
    let c = colors();
    view! {
        <div
            id="sidebar"
            style:display="flex"
            style:flex-direction="column"
            style:box-sizing="border-box"
            style:width="240px"
            style:flex-shrink="0"
            style:background-color={c.sidebar}
            style:border-right-width="1"
            style:border-right-style="solid"
            style:border-right-color={c.border_line}
        >
            <div
                style:padding="12px 12px 8px 12px"
                style:font-size="12px"
                style:font-weight="600"
                style:color={c.muted_foreground}
            >
                "REPOSITORIES"
            </div>
            // Enabled by the repository-list change. div-gap: 7: every
            // Button prop is given, because optional props are not filled.
            <div style:display="flex" style:gap="8px" style:padding="0 12px 12px 12px">
                <Button variant={ButtonVariant::Outline} size={ButtonSize::Sm} disabled={true}>
                    "Add"
                </Button>
                <Button variant={ButtonVariant::Outline} size={ButtonSize::Sm} disabled={true}>
                    "Scan folder"
                </Button>
            </div>
            <div id="repo-empty" style:padding="12px" style:color={c.muted_foreground}>
                "No repositories yet. Add one, or scan a folder for them."
            </div>
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
                    {format!("{}: select a repository.", tab.label())}
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
            <div
                id="toolbar"
                style:display="flex"
                style:align-items="center"
                style:height="48px"
                style:padding="0 16px"
                style:border-bottom-width="1"
                style:border-bottom-style="solid"
                style:border-bottom-color={c.border_line}
                style:font-weight="600"
            >
                "No repository selected"
            </div>
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
