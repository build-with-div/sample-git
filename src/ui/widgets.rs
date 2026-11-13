//! Small building blocks over div-components.

use div::prelude::*;
use div::typed_style::StyleProperty;
use div_components::{Button, ButtonProps, ButtonSize, ButtonVariant};

/// A div-components `Button` that runs `on_click` and is enabled while
/// `enabled()` is true.
///
/// div-gap: 5. `on:click` on a component tag is dropped by `view!` and
/// `Button` has no click prop, so the listener goes on the button element
/// itself.
/// div-gap: 7. Every `ButtonProps` field is given; optional props are not
/// filled in yet.
/// div-gap: 23. `Button`'s `disabled` is fixed when it is built and
/// `view!` attributes are not reactive, so an effect toggles the
/// attribute and the disabled styles the component would have set.
pub fn action_button(
    label: impl Into<String>,
    variant: ButtonVariant,
    size: ButtonSize,
    id: Option<String>,
    enabled: impl Fn() -> bool + 'static,
    on_click: impl Fn() + 'static,
) -> ViewNode {
    let node = Button(ButtonProps {
        variant,
        size,
        disabled: false,
        children: vec![ViewNode::Text(label.into())],
    });
    let ViewNode::Element(button) = &node else {
        unreachable!("Button builds one element");
    };
    if let Some(id) = id {
        button.set_attribute("id", &id).expect("set id");
    }
    button
        .on("click", move |event: &Event| {
            // A button inside a clickable row acts alone.
            event.stop_propagation();
            on_click();
        })
        .expect("listen for clicks");
    let weak = button.downgrade();
    create_effect(move || {
        let enabled = enabled();
        if let Some(button) = weak.upgrade() {
            set_enabled(&button, enabled);
        }
    });
    node
}

fn set_enabled(button: &Element, enabled: bool) {
    if enabled {
        let _ = button.remove_attribute("disabled");
        let _ = button.remove_property(StyleProperty::PointerEvents);
        let _ = button.remove_property(StyleProperty::Opacity);
    } else {
        button.set_attribute("disabled", "").expect("set disabled");
        button
            .apply_style(&style! { pointer-events: none; opacity: 0.5 })
            .expect("style disabled");
    }
}
