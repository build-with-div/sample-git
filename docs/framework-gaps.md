# Framework gaps

Places where Div (pinned in `Cargo.toml`) could not do what this app needs,
found while building it. Each entry says what was tried, what happened, which
part of Div it likely belongs to, how bad it is, and the workaround used here.

Entries keep the row numbers of the
[design's capability table](design.md#capability-gaps). Each workaround in the
code carries a `div-gap: <number>` comment, so it can be found and removed
when Div fixes the gap: `git grep -n "div-gap: 6"`.

Areas: **engine** (style, layout, paint, text), **framework** (divui,
platform, macros, CLI), **components** (div-components).

Pinned Div: `03573e7d`.

| # | Gap | Area | Severity | Status |
| --- | --- | --- | --- | --- |
| [3](#3-dynamic-style-values-need-typed-values) | Dynamic style values need typed values | framework: macros | workaround | open |
| [6](#6-for-and-show-cannot-be-children-in-view) | `For`/`Show` cannot be children in `view!` | framework: macros | workaround | open |
| [7](#7-optional-component-props-are-not-filled) | Optional component props are not filled | framework: macros | workaround | open |

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
