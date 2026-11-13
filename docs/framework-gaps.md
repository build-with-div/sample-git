# Framework gaps

Places where Div (pinned in `Cargo.toml`) could not do what this app needs,
found while building it. Each entry says what was tried, what happened, which
part of Div it likely belongs to, how bad it is, and the workaround used here.

Entries keep the row numbers of the
[design's capability table](design.md#capability-gaps); gaps found during
implementation continue from 23. Each workaround in the
code carries a `div-gap: <number>` comment, so it can be found and removed
when Div fixes the gap: `git grep -n "div-gap: 6"`.

Areas: **engine** (style, layout, paint, text), **framework** (divui,
platform, macros, CLI), **components** (div-components).

Pinned Div: `03573e7d`.

| # | Gap | Area | Severity | Status |
| --- | --- | --- | --- | --- |
| [1](#1-no-hover-styling) | No `:hover` styling | engine: style | workaround | open |
| [3](#3-dynamic-style-values-need-typed-values) | Dynamic style values need typed values | framework: macros | workaround | open |
| [5](#5-onclick-on-a-component-is-silently-dropped) | `on:click` on a component is silently dropped | framework: macros; components | workaround | open |
| [6](#6-for-and-show-cannot-be-children-in-view) | `For`/`Show` cannot be children in `view!` | framework: macros | workaround | open |
| [7](#7-optional-component-props-are-not-filled) | Optional component props are not filled | framework: macros | workaround | open |
| [20](#20-no-app-data-directory) | No app data directory | framework: platform | workaround | open |
| [21](#21-no-window-activation-event) | No window activation event | framework: platform | workaround | open |
| [23](#23-dynamic-attributes-are-set-once-and-disabledfalse-disables) | Dynamic attributes are set once, and `disabled={false}` disables | framework: macros | workaround | open |
| [24](#24-a-fresh-lockfile-does-not-build-on-linux) | A fresh lockfile does not build on Linux | framework: packaging | blocks Linux builds | open |

## 1. No hover styling

Div has no `:hover` (engine drafts #16–#19). The capability probe showed
that `mouseenter`/`mouseleave` fire and a dynamic `style:` repaints when its
signal changes.

Workaround: each sidebar row keeps a `hovered` signal set by
`on:mouseenter`/`on:mouseleave`, and its background is a dynamic style
(`ui/sidebar.rs`).

## 3. Dynamic style values need typed values

Tried:

```rust
view! { <div style:background-color={if on.get() { "rgb(255, 0, 0)" } else { "white" }}></div> }
view! { <div style:width={format!("{}px", width.get())}></div> }
```

Happened: `expected Color, found &str` and `expected LengthValue, found
String`. A static `style:x="…"` takes CSS text; a dynamic one takes only the
typed value. The types also differ between properties that take the same
CSS value: `background-color` takes a `Color`, `border-right-color` a
`StyleColor` (`StyleColor::Resolved(color)`).

Workaround: `ui/style.rs` converts the theme tokens to typed colors once
(`Colors`, with `border_line` for border colors), and dynamic lengths use
`LengthValue::px`.

## 5. `on:click` on a component is silently dropped

Tried:

```rust
view! { <Button variant={ButtonVariant::Default} size={ButtonSize::Default} disabled={false}
            on:click={move |_| commit()}>"Commit"</Button> }
```

Happened: it compiles and the handler never runs. `view!` keeps only
`name=value` attributes of component tags and drops `on:` ones without an
error, and `div_components::Button` has no click prop. (Probe: 0 clicks;
a wrapping `<span on:click>` received the bubbled click.)

Workaround: `ui::widgets::action_button` builds the `Button`, takes the
element out of the returned `ViewNode::Element`, and adds the listener with
`Element::on("click", …)`.

## 6. `For` and `Show` cannot be children in `view!`

Tried:

```rust
view! { <div>{For(move || items.get(), |s| s.clone(), |s| view! { <div>{s}</div> })}</div> }
```

Happened: a `{expr}` child is always converted with `ToString` into a text
node, so a `ViewNode` cannot be a child (here it does not compile).

Workaround: a component that returns its prop, `ui::style::Slot`:

```rust
#[component]
pub fn Slot(view: ViewNode) -> ViewNode { view }
// <Slot view={For(...)} />
```

The capability probe showed keyed `For` and `Show` update correctly inside it.

## 7. Optional component props are not filled

Tried: `<Button variant={ButtonVariant::Outline}>"Add"</Button>`.

Happened: the props struct literal needs every field (`size` and `disabled`
missing). RFC 0003 (c) is changing `view!` to a builder that fills them.

Workaround: every prop is given at each call site.

## 20. No app data directory

Div has no API for a per-user app data or config folder.

Workaround: `store.rs` reads `%APPDATA%` on Windows and `$XDG_CONFIG_HOME`
or `~/.config` elsewhere.

## 21. No window activation event

Nothing in the public API reports that the window was activated, which
is when a git client usually refreshes (the user committed in a terminal
and switched back).

Workaround: the selected repository refreshes every 30 s, after each
operation, and on Repository → Refresh (F5) (`main.rs`).

## 23. Dynamic attributes are set once, and `disabled={false}` disables

Tried:

```rust
let on = create_signal(false);
view! {
    <button disabled={on.get()} on:click=…>"B"</button>
    <div title={if on.get() { "yes" } else { "no" }}>"T"</div>
}
```

Happened (probe, Div `03573e7d`): the button gets `disabled="false"`, which
in HTML means disabled, and a click does nothing. After `on.set(true)` the
`title` is still `"no"`: unlike `style:` and text, a dynamic attribute is
written once with `ToString` and not tracked. `div_components::Button`'s
`disabled` prop is also fixed when the button is built.

Workaround: `ui::widgets::action_button` runs an effect that sets or
removes the `disabled` attribute and the disabled styles (`pointer-events:
none; opacity: 0.5`) with `Element` methods.

## 24. A fresh lockfile does not build on Linux

Tried: depending on `divui` (features `desktop`, `menu`, `dialog`) by git
`rev` and letting Cargo resolve a new `Cargo.lock`.

Happened: on ubuntu-24.04 `zbus` fails to compile with 8 errors
(`E0425`/`E0433` from the `#[interface]` macro). `div-platform` pins
`zbus = "=5.13.0"` (its `dialog` feature), but zbus 5.13.0 asks only for
`zbus_macros ^5.13.0` and `zvariant ^5.9.0`, so a fresh resolve takes
`zbus_macros` 5.19.0 and `zvariant` 5.15.1, which do not build with it.
Div's own `Cargo.lock` holds `zbus_macros` 5.13.0, `zbus_names` 4.3.1 and
`zvariant` 5.9.0, so Div's CI never meets this mix. Windows builds are not
affected: zbus is Linux-only.

Area: framework, packaging (`div-platform`). Div could pin the macro crates
beside zbus (`zbus_macros = "=5.13.0"`, `zvariant = "=5.9.0"`) or move to a
zbus release that builds with the latest macros.

Workaround: `Cargo.lock` was replaced with Div's `Cargo.lock` at the pinned
commit and Cargo pruned it to this app's dependencies. Bumping the pin
should repeat this. Marked `div-gap: 24` in `Cargo.toml`.
