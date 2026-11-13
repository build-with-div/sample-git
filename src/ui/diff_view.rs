//! The diff viewer: old and new line numbers, colored added and removed
//! lines, hunk headers, in Geist Mono.
//!
//! Lines are built with `Element` calls rather than `view!`: each `{expr}`
//! in `view!` is a reactive text node with its own effect, which a diff of
//! a thousand static lines does not need.

use crate::git::diff::{FileDiff, LineKind};
use crate::state::DiffState;
use crate::ui::style::{colors, Slot, SlotProps};
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{ButtonSize, ButtonVariant};
use std::rc::Rc;

/// Lines shown before "Show full diff".
///
/// div-gap: 8. No virtualized list: each line costs about 1.7 ms to lay
/// out headless, so a long diff is cut here.
pub const LINE_LIMIT: usize = 500;

/// What a diff view shows, and the prefix of its element ids
/// (`{id}-header`, `{id}-lines`, `{id}-truncated`, `show-full-{id}`).
pub struct DiffPane {
    pub id: &'static str,
    pub diff: Signal<DiffState>,
    /// Show every line rather than the first [`LINE_LIMIT`].
    pub full: Signal<bool>,
    pub header: Rc<dyn Fn() -> String>,
    /// Shown while there is no diff.
    pub empty: &'static str,
}

pub fn diff_view(pane: DiffPane) -> ViewNode {
    let c = colors();
    let DiffPane {
        id,
        diff,
        full,
        header,
        empty,
    } = pane;
    let document = current_document();
    let lines = Element::create(&document, "div").expect("create the diff body");
    lines
        .set_attribute("id", &format!("{id}-lines"))
        .expect("set id");
    lines
        .apply_style(&style! {
            font-family: "Geist Mono";
            font-size: 12px;
            line-height: 18px;
            padding-bottom: 16px
        })
        .expect("style the diff body");
    let body = lines.clone();
    // div-gap: 25. An effect re-runs outside the document context, where
    // `current_document()` panics, so the document is captured here.
    create_effect(move || {
        let shown = diff.get();
        let full = full.get();
        let _ = body.remove_all_children();
        match shown {
            DiffState::Empty => message(&document, &body, empty),
            DiffState::Loading => message(&document, &body, "Loading…"),
            DiffState::Failed(error) => message(&document, &body, &error),
            DiffState::Ready(shown) => render(&document, &body, &shown, full),
        }
    });
    let truncated = Show(
        move || {
            !full.get()
                && matches!(diff.get(), DiffState::Ready(shown) if shown.line_count() > LINE_LIMIT)
        },
        || ViewNode::Empty,
        move || {
            let total = match diff.get() {
                DiffState::Ready(shown) => shown.line_count(),
                _ => 0,
            };
            let show_all = action_button(
                "Show full diff",
                ButtonVariant::Outline,
                ButtonSize::Xs,
                Some(format!("show-full-{id}")),
                || true,
                move || full.set(true),
            );
            view! {
                <div
                    id={format!("{id}-truncated")}
                    style:display="flex"
                    style:align-items="center"
                    style:gap="8px"
                    style:padding="8px 12px"
                    style:color={c.muted_foreground}
                >
                    {format!("Showing {LINE_LIMIT} of {total} lines.")}
                    <Slot view={show_all} />
                </div>
            }
        },
    );
    view! {
        <div id={id}>
            <div
                id={format!("{id}-header")}
                style:padding="6px 12px"
                style:font-weight="600"
                style:white-space="nowrap"
                style:overflow="hidden"
                style:text-overflow="ellipsis"
            >
                {header()}
            </div>
            <Slot view={truncated} />
            <Slot view={ViewNode::Element(lines)} />
        </div>
    }
}

fn message(document: &Document, body: &Element, text: &str) {
    let line = Element::create(document, "div").expect("create a message");
    line.apply_style(&style!(
        "padding: 12px; font-family: Geist; font-size: 14px; color: #737373"
    ))
    .expect("style a message");
    line.set_text(text).expect("set the message");
    body.append_child(&line).expect("append the message");
}

fn render(document: &Document, body: &Element, diff: &FileDiff, full: bool) {
    if diff.binary {
        return message(document, body, "Binary file; no text diff.");
    }
    if diff.hunks.is_empty() {
        return message(document, body, "No text changes.");
    }
    let limit = if full { usize::MAX } else { LINE_LIMIT };
    let mut shown = 0;
    'hunks: for hunk in &diff.hunks {
        if shown >= limit {
            break;
        }
        body.append_child(&row(document, None, None, &hunk.header, RowKind::Hunk))
            .expect("append a hunk header");
        shown += 1;
        for line in &hunk.lines {
            if shown >= limit {
                break 'hunks;
            }
            let (kind, prefix) = match line.kind {
                LineKind::Context => (RowKind::Context, " "),
                LineKind::Added => (RowKind::Added, "+"),
                LineKind::Removed => (RowKind::Removed, "-"),
                LineKind::NoNewline => (RowKind::Note, ""),
            };
            let text = format!("{prefix}{}", line.text);
            body.append_child(&row(document, line.old, line.new, &text, kind))
                .expect("append a diff line");
            shown += 1;
        }
    }
}

#[derive(Clone, Copy)]
enum RowKind {
    Hunk,
    Context,
    Added,
    Removed,
    Note,
}

fn row(
    document: &Document,
    old: Option<u32>,
    new: Option<u32>,
    text: &str,
    kind: RowKind,
) -> Element {
    let row = Element::create(document, "div").expect("create a diff row");
    row.apply_style(&style! { display: flex; white-space: pre })
        .expect("style a diff row");
    let background = match kind {
        RowKind::Hunk => Some(style!("background-color: #ddf4ff; color: #57606a")),
        RowKind::Added => Some(style!("background-color: #e6ffec")),
        RowKind::Removed => Some(style!("background-color: #ffebe9")),
        RowKind::Note => Some(style!("color: #57606a")),
        RowKind::Context => None,
    };
    if let Some(background) = background {
        row.apply_style(&background).expect("color a diff row");
    }
    for number in [old, new] {
        let cell = Element::create(document, "span").expect("create a line number");
        cell.apply_style(&style!(
            "width: 44px; flex-shrink: 0; padding-right: 8px; text-align: right; color: #8c959f"
        ))
        .expect("style a line number");
        if let Some(number) = number {
            cell.set_text(&number.to_string())
                .expect("set a line number");
        }
        row.append_child(&cell).expect("append a line number");
    }
    let content = Element::create(document, "span").expect("create a diff line");
    content
        .apply_style(&style! { padding-left: 8px })
        .expect("style a diff line");
    content.set_text(text).expect("set a diff line");
    row.append_child(&content).expect("append a diff line");
    row
}
