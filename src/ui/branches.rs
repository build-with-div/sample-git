//! The Branches tab: create a branch at HEAD, then the local branches with
//! switch and delete, then the remote branches with check out.

use crate::actions;
use crate::git::branch::Branch;
use crate::state::State;
use crate::ui::history::{now, relative_time};
use crate::ui::style::colors;
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{Badge, BadgeProps, BadgeVariant, ButtonSize, ButtonVariant};

/// div-gap: 10. Keys are matched by code; `Event::key()` is not public yet.
const ENTER: i32 = 13;

pub fn branches_panel(state: State) -> ViewNode {
    let local = For(
        move || state.branches.get().local().cloned().collect::<Vec<_>>(),
        // A row is rebuilt when anything it shows changes.
        |branch| format!("{branch:?}"),
        move |branch| local_row(state, branch),
    );
    let remote = For(
        move || {
            let branches = state.branches.get();
            branches
                .remote()
                .map(|branch| {
                    let tracked = branches.tracked_by(&branch.name).map(|b| b.name.clone());
                    (branch.clone(), tracked)
                })
                .collect::<Vec<_>>()
        },
        |(branch, tracked)| format!("{branch:?}{tracked:?}"),
        move |(branch, tracked)| remote_row(state, branch, tracked),
    );
    let note = Show(
        move || !branches_note(state).is_empty(),
        || ViewNode::Empty,
        move || {
            let c = colors();
            view! {
                <div id="branches-note" style:padding="8px" style:color={c.muted_foreground}>
                    {branches_note(state)}
                </div>
            }
        },
    );
    let no_remotes = Show(
        move || {
            let branches = state.branches.get();
            !branches.loading && branches.error.is_none() && branches.remote().next().is_none()
        },
        || ViewNode::Empty,
        move || {
            let c = colors();
            view! {
                <div id="no-remote-branches" style:padding="4px 8px" style:color={c.muted_foreground}>
                    "No remote branches. Fetch from a remote to see its branches."
                </div>
            }
        },
    );
    view! {
        // div-gap: 26. One absolutely positioned, scrolling column.
        <div
            id="branches"
            style:position="absolute"
            style:top="0"
            style:right="0"
            style:bottom="0"
            style:left="0"
            style:overflow-y="auto"
        >
            {new_branch(state)}
            {note}
            {header("local-branches-header", move || {
                format!("Local branches ({})", state.branches.get().local().count())
            })}
            {local}
            <div style:height="12px"></div>
            {header("remote-branches-header", move || {
                format!("Remote branches ({})", state.branches.get().remote().count())
            })}
            {remote}
            {no_remotes}
        </div>
    }
}

/// Under the "New branch" row while the list loads or failed.
fn branches_note(state: State) -> String {
    let branches = state.branches.get();
    if let Some(error) = branches.error {
        error
    } else if branches.loading {
        "Loading…".to_owned()
    } else {
        String::new()
    }
}

fn header(id: &'static str, text: impl Fn() -> String + 'static) -> ViewNode {
    let c = colors();
    view! {
        <div
            id={id}
            style:padding="8px 8px 4px 8px"
            style:font-size="12px"
            style:font-weight="600"
            style:color={c.muted_foreground}
        >
            {text()}
        </div>
    }
}

/// The name box and "Create branch", with where the branch will start.
fn new_branch(state: State) -> ViewNode {
    let c = colors();
    let input = view! {
        <input
            id="new-branch-name"
            type="text"
            placeholder="New branch name"
            style:box-sizing="border-box"
            style:width="280px"
            style:height="32px"
            style:padding="4px 10px"
            style:font-family="Geist"
            style:font-size="14px"
            style:color={c.foreground}
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
            on:input={move |event: &Event| {
                if let Some(target) = event.target() {
                    state.new_branch.set(target.control_value().ok().flatten().unwrap_or_default());
                }
            }}
            on:keydown={move |event: &Event| {
                if event.key_code == ENTER {
                    event.prevent_default();
                    actions::create_branch(state);
                }
            }}
        />
    };
    let ViewNode::Element(element) = &input else {
        unreachable!("an input is one element");
    };
    // The box follows `state.new_branch`, which creating a branch clears.
    let weak = element.downgrade();
    create_effect(move || {
        let text = state.new_branch.get();
        if let Some(input) = weak.upgrade() {
            if input.control_value().ok().flatten().unwrap_or_default() != text {
                let _ = input.set_control_value(&text);
            }
        }
    });
    let create = action_button(
        "Create branch",
        ButtonVariant::Default,
        ButtonSize::Sm,
        Some("create-branch".into()),
        move || state.can_create_branch(),
        move || actions::create_branch(state),
    );
    view! {
        <div
            id="new-branch"
            style:display="flex"
            style:align-items="center"
            style:gap="8px"
            style:padding="4px 8px 8px 8px"
        >
            {input}
            {create}
            <div id="new-branch-base" style:font-size="12px" style:color={c.muted_foreground} style:white-space="nowrap">
                {new_branch_base(state)}
            </div>
        </div>
    }
}

/// `from main at a1b2c3d`, `from detached HEAD at a1b2c3d`.
fn new_branch_base(state: State) -> String {
    let Some(status) = state.selected_status() else {
        return String::new();
    };
    let head = status
        .branch
        .head
        .unwrap_or_else(|| "detached HEAD".to_owned());
    match status.branch.oid {
        Some(oid) => format!("from {head} at {}", &oid[..oid.len().min(7)]),
        None => "The first commit starts the branch.".to_owned(),
    }
}

/// `origin/main ↑1 ↓2`, `origin/old (gone)`, `no upstream`.
fn tracking(branch: &Branch) -> String {
    let Some(upstream) = &branch.upstream else {
        return "no upstream".to_owned();
    };
    if branch.gone {
        return format!("{upstream} (gone)");
    }
    let mut text = upstream.clone();
    if branch.ahead > 0 {
        text.push_str(&format!(" ↑{}", branch.ahead));
    }
    if branch.behind > 0 {
        text.push_str(&format!(" ↓{}", branch.behind));
    }
    text
}

/// `a1b2c3d · Fix a bug · 3 days ago`
fn tip(branch: &Branch) -> String {
    format!(
        "{} · {} · {}",
        branch.short_hash,
        branch.subject,
        relative_time(now(), branch.time)
    )
}

/// A row's frame: hover background, then the name and tip on the left and
/// `trailing` on the right.
fn row(id: String, title: ViewNode, detail: String, trailing: ViewNode) -> ViewNode {
    let c = colors();
    let transparent = Color::from_rgba8(0, 0, 0, 0);
    // div-gap: 1. No :hover, so a signal drives the hover background.
    let hovered = create_signal(false);
    view! {
        <div
            id={id}
            style:display="flex"
            style:align-items="center"
            style:gap="12px"
            style:padding="6px 8px"
            style:border-radius="4px"
            style:background-color={if hovered.get() { c.sidebar_accent } else { transparent }}
            on:mouseenter={move |_| hovered.set(true)}
            on:mouseleave={move |_| hovered.set(false)}
        >
            <div style:flex-grow="1" style:min-width="0">
                <div style:white-space="nowrap" style:overflow="hidden" style:text-overflow="ellipsis">
                    {title}
                </div>
                <div
                    style:font-size="12px"
                    style:color={c.muted_foreground}
                    style:white-space="nowrap"
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                >
                    {detail.clone()}
                </div>
            </div>
            {trailing}
        </div>
    }
}

fn local_row(state: State, branch: Branch) -> ViewNode {
    let c = colors();
    let name = branch.name.clone();
    let label = name.clone();
    let title = if branch.current {
        view! {
            <span>
                <span style:font-weight="600">{label}</span>
                <span style:margin-left="8px">
                    <Badge variant={BadgeVariant::Secondary}>"current"</Badge>
                </span>
            </span>
        }
    } else {
        view! { <span style:font-weight="500">{label}</span> }
    };
    let buttons = if branch.current {
        ViewNode::Empty
    } else {
        let (switch_name, delete_name) = (name.clone(), name.clone());
        let switch = action_button(
            "Switch",
            ButtonVariant::Outline,
            ButtonSize::Xs,
            Some(format!("switch-{name}")),
            move || !state.busy.get(),
            move || actions::switch_branch(state, switch_name.clone()),
        );
        let delete = action_button(
            "Delete",
            ButtonVariant::Ghost,
            ButtonSize::Xs,
            Some(format!("delete-{name}")),
            move || !state.busy.get(),
            move || actions::ask_delete_branch(state, delete_name.clone()),
        );
        ViewNode::Fragment(vec![switch, delete])
    };
    let (detail, upstream) = (tip(&branch), tracking(&branch));
    let trailing = view! {
        <div style:display="flex" style:align-items="center" style:gap="8px" style:flex-shrink="0">
            <div
                id={format!("tracking-{name}")}
                style:font-size="12px"
                style:color={c.muted_foreground}
                style:white-space="nowrap"
            >
                {upstream}
            </div>
            {buttons}
        </div>
    };
    row(format!("branch-{name}"), title, detail, trailing)
}

fn remote_row(state: State, branch: Branch, tracked: Option<String>) -> ViewNode {
    let c = colors();
    let name = branch.name.clone();
    let (label, detail) = (name.clone(), tip(&branch));
    let title = view! { <span style:font-weight="500">{label}</span> };
    let trailing = match tracked {
        Some(local) => {
            let text = format!("tracked by {local}");
            view! {
            <div
                id={format!("tracked-{name}")}
                style:flex-shrink="0"
                style:font-size="12px"
                style:color={c.muted_foreground}
                style:white-space="nowrap"
            >
                {text}
            </div>
            }
        }
        None => {
            let wanted = name.clone();
            action_button(
                "Check out",
                ButtonVariant::Outline,
                ButtonSize::Xs,
                Some(format!("checkout-{name}")),
                move || !state.busy.get(),
                move || actions::check_out_remote(state, wanted.clone()),
            )
        }
    };
    row(format!("remote-{name}"), title, detail, trailing)
}

#[cfg(test)]
mod tests {
    use super::tracking;
    use crate::git::branch::Branch;

    fn branch(upstream: Option<&str>, ahead: u32, behind: u32, gone: bool) -> Branch {
        Branch {
            name: "topic".into(),
            remote: false,
            current: false,
            upstream: upstream.map(str::to_owned),
            ahead,
            behind,
            gone,
            short_hash: "abc1234".into(),
            subject: "Work".into(),
            time: 0,
        }
    }

    #[test]
    fn tracking_texts() {
        assert_eq!(tracking(&branch(None, 0, 0, false)), "no upstream");
        assert_eq!(
            tracking(&branch(Some("origin/topic"), 0, 0, false)),
            "origin/topic"
        );
        assert_eq!(
            tracking(&branch(Some("origin/topic"), 2, 1, false)),
            "origin/topic ↑2 ↓1"
        );
        assert_eq!(
            tracking(&branch(Some("origin/topic"), 0, 3, false)),
            "origin/topic ↓3"
        );
        assert_eq!(
            tracking(&branch(Some("origin/old"), 0, 0, true)),
            "origin/old (gone)"
        );
    }
}
