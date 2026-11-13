//! The Changes tab: staged and unstaged files beside the diff viewer.

use crate::actions;
use crate::git::status::{Change, Entry};
use crate::state::{FileSelection, State};
use crate::ui::diff_view::{diff_view, DiffPane};
use crate::ui::style::{colors, Slot, SlotProps};
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{ButtonSize, ButtonVariant};
use std::rc::Rc;

/// div-gap: 10. Keys are matched by code; `Event::key()` is not public yet.
const ENTER: i32 = 13;

pub fn changes_panel(state: State) -> ViewNode {
    let c = colors();
    view! {
        // div-gap: 26. Absolutely positioned panes rather than a flex row.
        <div id="changes" style:position="absolute" style:top="0" style:right="0" style:bottom="0" style:left="0">
            <div
                id="file-lists"
                style:position="absolute"
                style:top="0"
                style:bottom="160px"
                style:left="0"
                style:box-sizing="border-box"
                style:width="320px"
                style:overflow-y="auto"
                style:border-right-width="1"
                style:border-right-style="solid"
                style:border-right-color={c.border_line}
                style:padding-right="8px"
            >
                <Slot view={section(state, true)} />
                <Slot view={section(state, false)} />
            </div>
            <Slot view={commit_box(state)} />
            <div
                id="diff-pane"
                style:position="absolute"
                style:top="0"
                style:right="0"
                style:bottom="0"
                style:left="320px"
                style:overflow="auto"
            >
                <Slot view={changes_diff(state)} />
            </div>
        </div>
    }
}

/// The diff of the file selected in the lists.
fn changes_diff(state: State) -> ViewNode {
    diff_view(DiffPane {
        id: "diff",
        diff: state.diff,
        full: state.full_diff,
        header: Rc::new(move || match state.file.get() {
            Some(file) if file.staged => format!("{} (staged)", file.path),
            Some(file) if file.untracked => format!("{} (new file)", file.path),
            Some(file) => file.path,
            None => String::new(),
        }),
        empty: "Select a file to see its changes.",
    })
}

/// The message box and Commit button under the lists.
fn commit_box(state: State) -> ViewNode {
    let c = colors();
    // div-gap: 27. A textarea ignores its own padding and draws its
    // placeholder in the text color, so the border and padding are on a box
    // around it and the placeholder is a muted label shown while it is empty.
    let message = view! {
        <textarea
            id="commit-message"
            style:display="block"
            style:box-sizing="border-box"
            style:width="100%"
            style:height="100%"
            style:font-family="Geist"
            style:font-size="14px"
            style:color={c.foreground}
            on:input={move |event: &Event| {
                if let Some(target) = event.target() {
                    state.message.set(target.control_value().ok().flatten().unwrap_or_default());
                }
            }}
            on:keydown={move |event: &Event| {
                let modifiers = event.modifiers as u32;
                let primary = (Modifiers::CTRL | Modifiers::META).bits();
                if event.key_code == ENTER && modifiers & primary != 0 {
                    event.prevent_default();
                    actions::commit(state);
                }
            }}
        ></textarea>
    };
    let ViewNode::Element(textarea) = &message else {
        unreachable!("a textarea is one element");
    };
    // The box follows `state.message`, which a commit clears.
    let weak = textarea.downgrade();
    create_effect(move || {
        let text = state.message.get();
        if let Some(textarea) = weak.upgrade() {
            if textarea.control_value().ok().flatten().unwrap_or_default() != text {
                let _ = textarea.set_control_value(&text);
            }
        }
    });
    let focus = textarea.downgrade();
    let placeholder = Show(
        move || state.message.get().is_empty(),
        || ViewNode::Empty,
        move || {
            view! {
                <div
                    id="commit-placeholder"
                    style:position="absolute"
                    style:top="8px"
                    style:left="9px"
                    style:color={c.muted_foreground}
                    style:pointer-events="none"
                >
                    "Commit message"
                </div>
            }
        },
    );
    let editor = view! {
        <div
            id="commit-editor"
            style:position="relative"
            style:box-sizing="border-box"
            style:height="88px"
            style:padding="8px"
            // div-gap: 4. No `border-*` shorthands.
            style:border-top-width="1"
            style:border-top-style="solid"
            style:border-top-color={c.border_line}
            style:border-right-width="1"
            style:border-right-style="solid"
            style:border-right-color={c.border_line}
            style:border-bottom-width="1"
            style:border-bottom-style="solid"
            style:border-bottom-color={c.border_line}
            style:border-left-width="1"
            style:border-left-style="solid"
            style:border-left-color={c.border_line}
            style:border-radius="6px"
            on:click={move |_| {
                // A click on the padding focuses the message too.
                if let Some(textarea) = focus.upgrade() {
                    let _ = textarea.focus();
                }
            }}
        >
            <Slot view={message} />
            <Slot view={placeholder} />
        </div>
    };
    let commit = action_button(
        "Commit",
        ButtonVariant::Default,
        ButtonSize::Sm,
        Some("commit".into()),
        move || state.can_commit(),
        move || actions::commit(state),
    );
    view! {
        <div
            id="commit-box"
            style:position="absolute"
            style:bottom="0"
            style:left="0"
            style:box-sizing="border-box"
            style:width="320px"
            style:height="160px"
            style:padding="12px 12px 12px 0"
            style:border-top-width="1"
            style:border-top-style="solid"
            style:border-top-color={c.border_line}
            style:border-right-width="1"
            style:border-right-style="solid"
            style:border-right-color={c.border_line}
        >
            <Slot view={editor} />
            <div
                style:display="flex"
                style:align-items="center"
                style:justify-content="space-between"
                style:margin-top="8px"
            >
                <div style:font-size="12px" style:color={c.muted_foreground}>"Ctrl+Enter to commit"</div>
                <Slot view={commit} />
            </div>
        </div>
    }
}

/// The staged or unstaged list with its header and bulk button.
fn section(state: State, staged: bool) -> ViewNode {
    let c = colors();
    let entries = move || -> Vec<Entry> {
        state
            .selected_status()
            .map(|status| {
                status
                    .entries
                    .into_iter()
                    .filter(|entry| {
                        if staged {
                            entry.staged.is_some()
                        } else {
                            entry.unstaged.is_some()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let bulk = if staged {
        action_button(
            "Unstage all",
            ButtonVariant::Ghost,
            ButtonSize::Xs,
            Some("unstage-all".into()),
            move || !state.busy.get() && !entries().is_empty(),
            move || actions::unstage_all(state),
        )
    } else {
        action_button(
            "Stage all",
            ButtonVariant::Ghost,
            ButtonSize::Xs,
            Some("stage-all".into()),
            move || !state.busy.get() && !entries().is_empty(),
            move || actions::stage_all(state),
        )
    };
    // A changed letter (M → D) rebuilds the row.
    let rows = For(
        entries,
        move |entry| (entry.path.clone(), letter(entry, staged)),
        move |entry| file_row(state, entry, staged),
    );
    let title = if staged { "Staged" } else { "Changes" };
    view! {
        <div style:margin-bottom="12px">
            <div
                style:display="flex"
                style:align-items="center"
                style:height="28px"
                style:padding-left="4px"
                style:font-size="12px"
                style:font-weight="600"
                style:color={c.muted_foreground}
            >
                <div style:flex-grow="1">{format!("{title} ({})", entries().len())}</div>
                <Slot view={bulk} />
            </div>
            <Slot view={rows} />
        </div>
    }
}

fn change_of(entry: &Entry, staged: bool) -> Change {
    let change = if staged { entry.staged } else { entry.unstaged };
    change.expect("listed entries have a change on their side")
}

fn letter(entry: &Entry, staged: bool) -> &'static str {
    change_of(entry, staged).letter()
}

pub(crate) fn letter_color(change: Change) -> Color {
    match change {
        Change::Added | Change::Untracked => Color::from_rgba8(26, 127, 55, 255),
        Change::Deleted => Color::from_rgba8(207, 34, 46, 255),
        Change::Conflicted => Color::from_rgba8(191, 135, 0, 255),
        _ => Color::from_rgba8(154, 103, 0, 255),
    }
}

fn file_row(state: State, entry: Entry, staged: bool) -> ViewNode {
    let c = colors();
    let transparent = Color::from_rgba8(0, 0, 0, 0);
    let change = change_of(&entry, staged);
    let path = entry.path.clone();
    let selection = FileSelection {
        path: path.clone(),
        staged,
        untracked: change == Change::Untracked,
    };
    let is_selected = {
        let selection = selection.clone();
        move || {
            state
                .file
                .get()
                .is_some_and(|file| file.path == selection.path && file.staged == selection.staged)
        }
    };
    // div-gap: 1. No :hover, so a signal drives the hover background.
    let hovered = create_signal(false);
    let side = if staged { "staged" } else { "unstaged" };
    let toggle_path = path.clone();
    let toggle = action_button(
        if staged { "Unstage" } else { "Stage" },
        ButtonVariant::Ghost,
        ButtonSize::Xs,
        Some(format!(
            "{}-{path}",
            if staged { "unstage" } else { "stage" }
        )),
        move || !state.busy.get(),
        move || {
            if staged {
                actions::unstage(state, toggle_path.clone());
            } else {
                actions::stage(state, toggle_path.clone());
            }
        },
    );
    // Unstaged rows can throw their changes away, after a confirmation.
    let discard = if staged {
        ViewNode::Empty
    } else {
        let discard_path = path.clone();
        let untracked = change == Change::Untracked;
        action_button(
            if untracked { "Delete" } else { "Discard" },
            ButtonVariant::Ghost,
            ButtonSize::Xs,
            Some(format!("discard-{path}")),
            move || !state.busy.get(),
            move || actions::ask_discard(state, discard_path.clone(), untracked),
        )
    };
    let label = match &entry.orig_path {
        Some(orig) => format!("{orig} → {path}"),
        None => path.clone(),
    };
    view! {
        <div
            id={format!("file-{side}-{path}")}
            style:display="flex"
            style:align-items="center"
            style:gap="8px"
            style:padding="2px 4px"
            style:border-radius="4px"
            style:background-color={if is_selected() {
                c.border
            } else if hovered.get() {
                c.sidebar_accent
            } else {
                transparent
            }}
            on:click={move |_| actions::select_file(state, selection.clone())}
            on:mouseenter={move |_| hovered.set(true)}
            on:mouseleave={move |_| hovered.set(false)}
        >
            <span
                style:width="12px"
                style:flex-shrink="0"
                style:font-family="Geist Mono"
                style:font-size="12px"
                style:font-weight="600"
                style:color={letter_color(change)}
            >
                {change.letter()}
            </span>
            <span
                style:flex-grow="1"
                style:min-width="0"
                style:overflow="hidden"
                style:text-overflow="ellipsis"
                style:white-space="nowrap"
            >
                {label.clone()}
            </span>
            <Slot view={discard} />
            <Slot view={toggle} />
        </div>
    }
}
