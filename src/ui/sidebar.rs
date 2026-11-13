//! The repository list.

use crate::actions;
use crate::state::{Repo, State};
use crate::ui::style::{colors, Slot, SlotProps};
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{ButtonSize, ButtonVariant};

pub fn sidebar(state: State) -> ViewNode {
    let c = colors();
    let add = action_button(
        "Add",
        ButtonVariant::Outline,
        ButtonSize::Sm,
        Some("add-repo".into()),
        || true,
        move || actions::pick_and_add(state),
    );
    let scan = action_button(
        "Scan folder",
        ButtonVariant::Outline,
        ButtonSize::Sm,
        Some("scan-folder".into()),
        || true,
        move || actions::pick_and_scan(state),
    );
    let rows = For(
        move || state.repos.get(),
        |repo| repo.path.clone(),
        move |repo| row(state, repo),
    );
    let empty = Show(
        move || state.repos.get().is_empty(),
        || ViewNode::Empty,
        move || {
            view! {
                <div id="repo-empty" style:padding="4px 12px" style:color={c.muted_foreground}>
                    "No repositories yet. Add one, or scan a folder for them."
                </div>
            }
        },
    );
    view! {
        <div
            id="sidebar"
            style:position="absolute"
            style:top="0"
            style:bottom="24px"
            style:left="0"
            style:box-sizing="border-box"
            style:width="240px"
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
            <div style:display="flex" style:gap="8px" style:padding="0 12px 12px 12px">
                <Slot view={add} />
                <Slot view={scan} />
            </div>
            <div
                id="repo-list"
                style:position="absolute"
                style:top="81px"
                style:right="0"
                style:bottom="0"
                style:left="0"
                style:overflow-y="auto"
            >
                <Slot view={rows} />
                <Slot view={empty} />
            </div>
        </div>
    }
}

fn row(state: State, repo: Repo) -> ViewNode {
    let c = colors();
    let transparent = Color::from_rgba8(0, 0, 0, 0);
    // div-gap: 1. No :hover, so a signal drives the hover background.
    let hovered = create_signal(false);
    let path = repo.path.clone();
    let is_selected = {
        let path = path.clone();
        move || state.selected.get().as_deref() == Some(path.as_path())
    };
    let summary_path = path.clone();
    let remove_path = path.clone();
    let remove = action_button(
        "×",
        ButtonVariant::Ghost,
        ButtonSize::Xs,
        Some(format!("remove-{}", repo.name)),
        || true,
        move || actions::remove_repo(state, &remove_path),
    );
    let background = {
        let is_selected = is_selected.clone();
        move || {
            if is_selected() {
                c.border
            } else if hovered.get() {
                c.sidebar_accent
            } else {
                transparent
            }
        }
    };
    let select_path = path.clone();
    view! {
        <div
            id={format!("repo-{}", repo.name)}
            style:display="flex"
            style:align-items="center"
            style:gap="4px"
            style:margin="0 8px 2px 8px"
            style:padding="6px 4px 6px 8px"
            style:border-radius="6px"
            style:background-color={background()}
            on:click={move |_| state.selected.set(Some(select_path.clone()))}
            on:mouseenter={move |_| hovered.set(true)}
            on:mouseleave={move |_| hovered.set(false)}
        >
            <div style:flex-grow="1" style:min-width="0">
                <div
                    style:font-weight="500"
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                    style:white-space="nowrap"
                >
                    {repo.name.clone()}
                </div>
                <div
                    style:font-size="12px"
                    style:color={c.muted_foreground}
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                    style:white-space="nowrap"
                >
                    {state.summary(&summary_path).line()}
                </div>
            </div>
            <Slot view={remove} />
        </div>
    }
}
