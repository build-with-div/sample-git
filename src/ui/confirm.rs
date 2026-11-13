//! The dialog: a question before an action that can lose or publish work,
//! or a message such as git's full error output.
//!
//! div-gap: 14. Div has no dialog or top layer, so this is a scrim over the
//! window with a card in the middle. `Document::set_modal_root` keeps input
//! inside it, and Escape cancels.

use crate::actions;
use crate::state::State;
use crate::ui::style::colors;
use crate::ui::widgets::action_button;
use div::prelude::*;
use div_components::{ButtonSize, ButtonVariant};

/// div-gap: 10. Keys are matched by code; `Event::key()` is not public yet.
const ESCAPE: i32 = 27;

pub fn confirm_dialog(state: State) -> ViewNode {
    // div-gap: 25. Effects re-run outside the document context.
    let document = current_document();
    create_effect({
        let document = document.clone();
        move || {
            if state.confirm.get().is_none() {
                let _ = document.set_modal_root(None);
            }
        }
    });
    Show(
        move || state.confirm.get().is_some(),
        || ViewNode::Empty,
        move || dialog(state, document.clone()),
    )
}

fn dialog(state: State, document: Document) -> ViewNode {
    let c = colors();
    let Some(confirm) = state.confirm.get() else {
        return ViewNode::Empty;
    };
    // A question has Cancel; a message has only its one button.
    let cancel = confirm.run.as_ref().map(|_| {
        action_button(
            "Cancel",
            ButtonVariant::Outline,
            ButtonSize::Default,
            Some("confirm-cancel".into()),
            || true,
            move || actions::cancel_confirm(state),
        )
    });
    let accept = action_button(
        confirm.action.clone(),
        if confirm.destructive {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Default
        },
        ButtonSize::Default,
        Some("confirm-accept".into()),
        || true,
        move || actions::accept_confirm(state),
    );
    // Focus Cancel, so Enter does not run the action by accident; a
    // message focuses its one button.
    let ViewNode::Element(focused) = cancel.as_ref().unwrap_or(&accept) else {
        unreachable!("a button is one element");
    };
    let focused = focused.clone();
    let cancel = cancel.unwrap_or(ViewNode::Empty);
    // div-gap: 30. No `<pre>` element (an unknown tag fails at run time),
    // so a div takes its monospace font and preserved white space.
    let detail = match confirm.detail.clone() {
        Some(text) => view! {
            <div
                id="confirm-detail"
                style:box-sizing="border-box"
                style:max-height="240px"
                style:overflow="auto"
                style:margin-top="12px"
                style:padding="8px 12px"
                style:border-radius="6px"
                style:font-family="Geist Mono"
                style:font-size="12px"
                style:line-height="18px"
                style:white-space="pre-wrap"
                style:background-color={c.muted}
            >
                {text}
            </div>
        },
        None => ViewNode::Empty,
    };
    let scrim = view! {
        <div
            id="confirm"
            role="alertdialog"
            style:position="absolute"
            style:top="0"
            style:right="0"
            style:bottom="0"
            style:left="0"
            style:z-index="10"
            style:display="flex"
            style:align-items="center"
            style:justify-content="center"
            style:background-color="rgba(0, 0, 0, 0.5)"
            on:keydown={move |event: &Event| {
                if event.key_code == ESCAPE {
                    event.prevent_default();
                    actions::cancel_confirm(state);
                }
            }}
        >
            <div
                style:box-sizing="border-box"
                style:width="440px"
                style:padding="24px"
                style:border-radius="8px"
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
                style:background-color={c.background}
            >
                <div id="confirm-title" style:font-size="18px" style:line-height="28px" style:font-weight="600">
                    {confirm.title.clone()}
                </div>
                <div id="confirm-body" style:margin-top="8px" style:color={c.muted_foreground}>
                    {confirm.body.clone()}
                </div>
                {detail}
                <div style:display="flex" style:justify-content="flex-end" style:gap="8px" style:margin-top="24px">
                    {cancel}
                    {accept}
                </div>
            </div>
        </div>
    };
    if let ViewNode::Element(root) = &scrim {
        // Once the dialog is in the document: keep input in it and move
        // focus into it.
        let root = root.clone();
        div::task::spawn_local(async move {
            let _ = document.set_modal_root(Some(&root));
            let _ = focused.focus();
        })
        .detach();
    }
    scrim
}
