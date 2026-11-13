//! The History tab: the commits of HEAD beside the selected commit's
//! message, metadata, changed files and diff.

use crate::actions;
use crate::git::log::{ChangedFile, Commit, CommitDetails};
use crate::state::{Loadable, State};
use crate::ui::changes::letter_color;
use crate::ui::diff_view::{diff_view, DiffPane};
use crate::ui::style::colors;
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{ButtonSize, ButtonVariant};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn history_panel(state: State) -> ViewNode {
    let c = colors();
    view! {
        // div-gap: 26. Absolutely positioned panes rather than a flex row.
        <div id="history" style:position="absolute" style:top="0" style:right="0" style:bottom="0" style:left="0">
            <div
                id="commit-list"
                style:position="absolute"
                style:top="0"
                style:bottom="0"
                style:left="0"
                style:box-sizing="border-box"
                style:width="320px"
                style:overflow-y="auto"
                style:padding-right="8px"
                style:border-right-width="1"
                style:border-right-style="solid"
                style:border-right-color={c.border_line}
            >
                {commit_list(state)}
            </div>
            <div
                id="commit-pane"
                style:position="absolute"
                style:top="0"
                style:right="0"
                style:bottom="0"
                style:left="320px"
                style:overflow="auto"
            >
                {commit_pane(state)}
            </div>
        </div>
    }
}

/// The commits, then "Load more", "Loading…" or why there are none.
fn commit_list(state: State) -> ViewNode {
    let c = colors();
    // div-gap: 8. No virtualized list: History loads 200 commits at a time.
    let rows = For(
        move || state.history.get().commits,
        |commit| commit.hash.clone(),
        move |commit| commit_row(state, commit),
    );
    let footer = Show(
        move || state.history.get().more,
        move || {
            view! {
                <div id="history-note" style:padding="8px" style:color={c.muted_foreground}>
                    {history_note(state)}
                </div>
            }
        },
        move || {
            let more = action_button(
                "Load more",
                ButtonVariant::Outline,
                ButtonSize::Sm,
                Some("load-more".into()),
                move || !state.history.get().loading,
                move || actions::load_more(state),
            );
            view! {
                <div style:padding="8px">
                    {more}
                </div>
            }
        },
    );
    view! {
        <div style:padding-top="4px">
            {rows}
            {footer}
        </div>
    }
}

/// Under the list when there is nothing more to load.
fn history_note(state: State) -> String {
    let history = state.history.get();
    if history.loading {
        "Loading…".to_owned()
    } else if let Some(error) = history.error {
        error
    } else if history.commits.is_empty() {
        "No commits yet.".to_owned()
    } else {
        String::new()
    }
}

fn commit_row(state: State, commit: Commit) -> ViewNode {
    let c = colors();
    let transparent = Color::from_rgba8(0, 0, 0, 0);
    let hash = commit.hash.clone();
    let selected = {
        let hash = hash.clone();
        move || state.commit.get().as_ref() == Some(&hash)
    };
    // div-gap: 1. No :hover, so a signal drives the hover background.
    let hovered = create_signal(false);
    let line = format!(
        "{} · {} · {}",
        commit.short,
        commit.author,
        relative_time(now(), commit.time)
    );
    view! {
        <div
            id={format!("commit-{}", commit.short)}
            style:padding="6px 8px"
            style:border-radius="4px"
            style:background-color={if selected() {
                c.border
            } else if hovered.get() {
                c.sidebar_accent
            } else {
                transparent
            }}
            on:click={move |_| actions::select_commit(state, hash.clone())}
            on:mouseenter={move |_| hovered.set(true)}
            on:mouseleave={move |_| hovered.set(false)}
        >
            <div
                style:font-weight="500"
                style:overflow="hidden"
                style:text-overflow="ellipsis"
                style:white-space="nowrap"
            >
                {commit.subject.clone()}
            </div>
            <div
                style:font-size="12px"
                style:color={c.muted_foreground}
                style:overflow="hidden"
                style:text-overflow="ellipsis"
                style:white-space="nowrap"
            >
                {line.clone()}
            </div>
        </div>
    }
}

/// The selected commit, or what to do to see one.
fn commit_pane(state: State) -> ViewNode {
    let pane = DynChild(move || match state.details.get() {
        Loadable::Ready(details) => commit_details(state, details),
        Loadable::Empty => note("Select a commit to see its details."),
        Loadable::Loading => note("Loading…"),
        Loadable::Failed(error) => note(&error),
    });
    // div-gap: 28. DynChild wraps its content in a `<span>`; take it out
    // of layout.
    if let ViewNode::Element(span) = &pane {
        span.apply_style(&style! { display: contents })
            .expect("style the pane");
    }
    pane
}

fn note(text: &str) -> ViewNode {
    let c = colors();
    let text = text.to_owned();
    view! {
        <div id="commit-note" style:padding="12px" style:color={c.muted_foreground}>
            {text.clone()}
        </div>
    }
}

fn commit_details(state: State, details: CommitDetails) -> ViewNode {
    let c = colors();
    let commit = details.commit.clone();
    let subject = commit.subject.clone();
    let body = details
        .message
        .split_once('\n')
        .map(|(_, rest)| rest.trim().to_owned())
        .unwrap_or_default();
    let parents = commit
        .parents
        .iter()
        .map(|parent| parent.chars().take(commit.short.len()).collect::<String>())
        .collect::<Vec<_>>()
        .join(", ");
    let meta = [
        ("Author", format!("{} <{}>", commit.author, commit.email)),
        ("Date", details.date.clone()),
        ("Commit", commit.hash.clone()),
        (
            "Parents",
            if parents.is_empty() {
                "none (root commit)".to_owned()
            } else {
                parents
            },
        ),
    ];
    let meta: Vec<ViewNode> = meta
        .into_iter()
        .map(|(label, value)| {
            view! {
                <div style:display="flex" style:gap="12px" style:margin-top="4px">
                    <div style:width="56px" style:flex-shrink="0" style:font-size="12px" style:color={c.muted_foreground}>
                        {label}
                    </div>
                    <div id={format!("commit-{}", label.to_lowercase())} style:min-width="0">{value}</div>
                </div>
            }
        })
        .collect();
    let count = details.files.len();
    let files: Vec<ViewNode> = details
        .files
        .into_iter()
        .map(|file| commit_file_row(state, file))
        .collect();
    let diff = diff_view(DiffPane {
        id: "commit-diff",
        diff: state.commit_diff,
        full: state.commit_full_diff,
        header: Rc::new(move || state.commit_file.get().unwrap_or_default()),
        empty: "Select a file to see its changes in this commit.",
    });
    view! {
        <div id="commit-details">
            <div style:padding="12px 12px 0 12px">
                <div id="commit-subject" style:font-size="16px" style:line-height="24px" style:font-weight="600">
                    {subject.clone()}
                </div>
                <div id="commit-body" style:margin-top="4px" style:white-space="pre-wrap">{body}</div>
                <div style:margin-top="12px">
                    {meta}
                </div>
                <div
                    style:margin-top="16px"
                    style:font-size="12px"
                    style:font-weight="600"
                    style:color={c.muted_foreground}
                >
                    {format!("Changed files ({count})")}
                </div>
                <div style:margin-top="4px" style:margin-bottom="12px">
                    {files}
                </div>
            </div>
            {diff}
        </div>
    }
}

fn commit_file_row(state: State, file: ChangedFile) -> ViewNode {
    let c = colors();
    let transparent = Color::from_rgba8(0, 0, 0, 0);
    let path = file.path.clone();
    let selected = {
        let path = path.clone();
        move || state.commit_file.get().as_ref() == Some(&path)
    };
    let label = match &file.orig_path {
        Some(orig) => format!("{orig} → {}", file.path),
        None => file.path.clone(),
    };
    view! {
        <div
            id={format!("commit-file-{}", file.path)}
            style:display="flex"
            style:gap="8px"
            style:padding="2px 4px"
            style:border-radius="4px"
            style:background-color={if selected() { c.border } else { transparent }}
            on:click={move |_| actions::select_commit_file(state, path.clone())}
        >
            <span
                style:width="12px"
                style:flex-shrink="0"
                style:font-family="Geist Mono"
                style:font-size="12px"
                style:font-weight="600"
                style:color={letter_color(file.change)}
            >
                {file.change.letter()}
            </span>
            <span style:min-width="0" style:overflow="hidden" style:text-overflow="ellipsis" style:white-space="nowrap">
                {label.clone()}
            </span>
        </div>
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

/// `just now`, `5 minutes ago`, `3 days ago`, `2 years ago`.
pub fn relative_time(now: i64, then: i64) -> String {
    let seconds = (now - then).max(0);
    let (count, unit) = match seconds {
        0..=59 => return "just now".to_owned(),
        60..=3_599 => (seconds / 60, "minute"),
        3_600..=86_399 => (seconds / 3_600, "hour"),
        86_400..=2_591_999 => (seconds / 86_400, "day"),
        2_592_000..=31_535_999 => (seconds / 2_592_000, "month"),
        _ => (seconds / 31_536_000, "year"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

#[cfg(test)]
mod tests {
    use super::relative_time;

    #[test]
    fn relative_times() {
        let now = 1_000_000_000;
        assert_eq!(relative_time(now, now), "just now");
        assert_eq!(relative_time(now, now + 30), "just now");
        assert_eq!(relative_time(now, now - 60), "1 minute ago");
        assert_eq!(relative_time(now, now - 7_200), "2 hours ago");
        assert_eq!(relative_time(now, now - 3 * 86_400), "3 days ago");
        assert_eq!(relative_time(now, now - 40 * 86_400), "1 month ago");
        assert_eq!(relative_time(now, now - 800 * 86_400), "2 years ago");
    }
}
