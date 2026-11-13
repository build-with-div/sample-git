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

The workarounds are temporary scaffolding. The fix for each gap is the web
platform feature that makes it unnecessary, made to match Chromium, so that
code written as it would be for the browser (or ported from an npm package)
works on Div unchanged. The "Removed by" column names that feature. Where
no web feature applies (macros, packaging, desktop services), it names the
JSX, Solid/Leptos or Electron/Tauri behavior the app expects instead.

Pinned Div: `92ea25a0`.

| # | Gap | Removed by | Area | Severity | Status |
| --- | --- | --- | --- | --- | --- |
| [1](#1-no-hover-styling) | No `:hover` styling | CSS `:hover` (with `:focus-visible`, `:active`) and hover-state style invalidation | engine: style | workaround | open |
| [3](#3-dynamic-style-values-need-typed-values) | Dynamic style values need typed values | CSS text for dynamic values, as `element.style.setProperty(name, value)` parses it | framework: macros | workaround | open |
| [4](#4-common-style-literals-are-rejected) | Common style literals are rejected | Chromium's CSS parsing: shorthands (`inset`, `flex`, `border`, `border-width`/`-style`/`-color`), `<length>` border widths (`1px`), `resize`, `cursor: col-resize` | engine: style; framework: macros | workaround | open |
| [5](#5-onclick-on-a-component-is-silently-dropped) | `on:click` on a component is silently dropped | JSX-style event props on components (fixed by forwarding) | framework: macros; components | — | fixed in `565db46a` |
| [6](#6-for-and-show-cannot-be-children-in-view) | `For`/`Show` cannot be children in `view!` | JSX expression children: a view (element, array, `.map()` result) as a `{expr}` child, as in React and Leptos | framework: macros | — | fixed in `92ea25a0` |
| [7](#7-optional-component-props-are-not-filled) | Optional component props are not filled | JSX props with defaults: omitted optional props take their default | framework: macros | workaround | open |
| [8](#8-no-virtualized-list) | No virtualized list | CSS `content-visibility: auto` with `contain-intrinsic-size`, and layout fast enough for react-window-style windowing (absolute rows driven by `scroll` events) | framework; components | workaround: long diffs are cut | open |
| [10](#10-keys-are-matched-by-code) | Keys are matched by code | `KeyboardEvent.key` and `ctrlKey`/`metaKey` (UI Events); key names landed in Div #27, workaround not removed yet | framework: events | workaround | open |
| [14](#14-no-dialog-or-top-layer) | No dialog or top layer | `<dialog>` with `showModal()`: top layer, `::backdrop`, inert background, Escape `cancel` event, focus moved into the dialog | framework: dialog; engine: top layer | workaround | open |
| [20](#20-no-app-data-directory) | No app data directory | Electron `app.getPath("userData")` / Tauri `appDataDir()`, and `shell.showItemInFolder` (no web feature) | framework: platform | workaround | open |
| [21](#21-no-window-activation-event) | No window activation event | `window` `focus`/`blur` events and `visibilitychange` (Page Visibility) | framework: platform | workaround | open |
| [23](#23-dynamic-attributes-are-set-once-and-disabledfalse-disables) | Dynamic attributes are set once, and `disabled={false}` disables | HTML boolean attributes with reactive binding: `disabled={false}` removes the attribute, as in React; attribute values update like `setAttribute`/`removeAttribute` | framework: macros | workaround | open |
| [24](#24-a-fresh-lockfile-does-not-build-on-linux) | A fresh lockfile does not build on Linux | None (Rust packaging): Div pins the zbus macro crates it needs | framework: packaging | blocks Linux builds | open |
| [25](#25-current_document-panics-when-an-effect-re-runs) | `current_document()` panics when an effect re-runs | Effects that run in the owner and context they were created in (Solid/Leptos owners); on the web `document` is always available | framework: reactive | workaround | open |
| [26](#26-layout-time-grows-exponentially-with-nested-flex-containers) | Layout time grows exponentially with nested flex containers | Chromium LayoutNG flex: cached layout results, so nested flexbox lays out in linear time | engine: layout | blocked the diff view; workaround | open |
| [27](#27-a-textarea-ignores-its-padding-and-greys-no-placeholder) | A textarea ignores its padding and greys no placeholder | `<textarea>` box model (padding inside the text box) and the `::placeholder` pseudo-element with Chromium's grey UA color | engine: forms | workaround | open |
| [28](#28-dynchild-wraps-its-content-in-a-span) | `DynChild` wraps its content in a `<span>` | Dynamic children that add no element, as JSX `{cond ? a : b}` in React, Solid and Leptos (markers or comment nodes, not a box) | framework: reactive | workaround | open |

Gaps the design's probe found that the app has not met in code yet, because
v1 left the feature out:

| # | Gap | Removed by | Area |
| --- | --- | --- | --- |
| 2 | Shared styles | CSS class selectors and stylesheets (`<style>`, the cascade) | engine: style |
| 15 | Context menus | The `contextmenu` event, plus a popup menu like Electron `Menu.popup` | framework: events, platform |
| 16 | Resizable split panes | Pointer Events (`pointerdown`/`pointermove`) with `setPointerCapture`, and `cursor: col-resize` | framework: events; engine: style |
| 17 | Icons | Inline SVG (`<svg>`, `<path>`, `stroke`), as lucide-react renders | engine: paint |
| 18 | `<select>` | HTML `<select>`: the first option selected by default, a popup list | engine: forms |
| 19 | Dark mode | The `prefers-color-scheme` media query and CSS custom properties (`var()`) | engine: style |
| 22 | Tooltips | The `title` attribute, or the Popover API with anchor positioning (as Radix Tooltip uses) | framework; engine |

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

## 4. Common style literals are rejected

Tried, in `view!` and `style!`:

```rust
style:inset="0"
style:flex="1"
style:cursor="col-resize"
style:border-width="1px"   style:border-style="solid"   style:border-color={c.border_line}
style:border-top-width="1px"
style:resize="none"
```

Happened: compile errors. `inset`, `flex`, `border-width`, `border-style`
and `border-color` are "unknown Div style property"; `cursor: col-resize`
and `resize: none` are invalid literals; `border-top-width` takes a bare
integer (`"1"`) and rejects `"1px"`, which CSS and Chromium accept.

Workaround: longhands everywhere (`top`/`right`/`bottom`/`left`,
`flex-grow`, the twelve `border-<side>-<part>` properties), unitless border
widths, no resize cursor, and the textarea keeps the default `resize`.
Marked `div-gap: 4` where a box needs all four borders.

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

Workaround (removed): `ui::widgets::action_button` built the `Button`, took
the element out of the returned `ViewNode::Element`, and added the listener
with `Element::on("click", …)`.

Fixed in Div `565db46a` (div#29, `b1cf9d58`): `view!` forwards `on:`
attributes on a component tag to the element it returns. `action_button`
now writes `<Button on:click={…}>`; its tests (every button in
`tests/ui_*.rs`) pass unchanged.

## 6. `For` and `Show` cannot be children in `view!`

Tried:

```rust
view! { <div>{For(move || items.get(), |s| s.clone(), |s| view! { <div>{s}</div> })}</div> }
```

Happened: a `{expr}` child is always converted with `ToString` into a text
node, so a `ViewNode` cannot be a child (here it does not compile).

Workaround (removed): a component that returned its prop, `ui::style::Slot`,
written `<Slot view={For(...)} />`. The capability probe showed keyed `For`
and `Show` update correctly inside it.

Fixed in Div `92ea25a0` (div#30, `6fdb0d6`): a `{expr}` child that is a
view (`ViewNode`, `Vec<ViewNode>`, `Option<ViewNode>`, `For`, `Show`) is
mounted as itself, and one that implements `ToString` stays reactive text.
Every `<Slot view={x} />` is now `{x}`, and the commit details pass their
`Vec<ViewNode>` rows directly; the UI tests pass unchanged.

## 7. Optional component props are not filled

Tried: `<Button variant={ButtonVariant::Outline}>"Add"</Button>`.

Happened: the props struct literal needs every field (`size` and `disabled`
missing). RFC 0003 (c) is changing `view!` to a builder that fills them.

Workaround: every prop is given at each call site.

## 8. No virtualized list

Tried: rendering every line of a diff, one row element per line, in a
scrolling pane.

Happened: there is no virtualized or windowed list in divui or
div-components, so every line is built and laid out. Headless, with the
frame from gap 26 in place, a 1,500-line diff took about 2.6 s to show
(`TestApp`, debug build of this crate, dependencies at `opt-level` 2), about
1.7 ms a line.

Workaround: `ui::diff_view::LINE_LIMIT` (500) lines are shown, with
"Show full diff" to build the rest. Rows are built with `Element` calls
rather than `view!`, so static lines carry no effects.

## 10. Keys are matched by code

Tried: Ctrl+Enter in the commit message box and Escape in the confirmation
dialog.

Happened: `on:keydown` works, but the `Event` gives `key_code` (13, 27),
`key_text` and a raw `modifiers` bitmask (`i32`, while `Modifiers` holds a
`u32`). Named keys (`Event::key()`) are in RFC 0003 (b) and not public.

Workaround: `ENTER` and `ESCAPE` constants and a cast of `modifiers`
against `Modifiers::CTRL | Modifiers::META`. Marked `div-gap: 10`.

## 14. No dialog or top layer

Tried: asking before discarding a file's changes or deleting an untracked
file.

Happened: there is no `<dialog>`, top layer or native message box in the
public API.

Workaround (`ui/confirm.rs`): a scrim absolutely positioned over the whole
window with `z-index: 10` and a card in the middle. After it mounts,
`Document::set_modal_root` limits input to it and Cancel takes focus;
Escape cancels. `tests/ui_commit.rs` checks that a click on a button under
the scrim does nothing. Marked `div-gap: 14`.

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

## 25. `current_document()` panics when an effect re-runs

Tried, in a component:

```rust
create_effect(move || {
    let diff = state.diff.get();
    let row = Element::create(&current_document(), "div").unwrap();
    // ...
});
```

Happened: the first run works; the next run, after `state.diff` changes,
panics with "current_document() called outside with_document()". Effects
re-run from the scheduler, outside the document context that component
code runs in, so `view!` and `current_document()` cannot be used in an
effect that builds elements.

Area: framework, reactive runtime. An effect could re-run inside the
document it was created in.

Workaround: `ui::diff_view` captures the `Document` when the component is
built and passes it to the helpers that create rows. Marked `div-gap: 25`.

## 26. Layout time grows exponentially with nested flex containers

Tried: the usual app frame of nested flex columns (window → row of sidebar
and main → main column → tabs column → panel → flex row of file list and
diff pane), with a diff of 1,500 rows in the pane.

Happened: clicking a file took 256 s headless before the diff showed, and
"Show full diff" (1,700 rows) another 290 s. A probe
([`docs/probes/flex_depth.rs`](probes/flex_depth.rs)) appends 100 rows
(a flex row with three spans each) to a scrolling element under `depth`
nested `display: flex; flex-direction: column; flex-grow: 1; min-height: 0`
containers, then calls `run_until_idle`:

| depth | 0 | 1 | 2 | 3 | 4 | 5 | 6 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| nested flex columns | 0.27 s | 0.56 s | 1.27 s | 2.89 s | 6.53 s | 14.9 s | 34.1 s |
| nested blocks (`height: 100%`) | 0.16 s | | | 0.15 s | | | 0.16 s |

Each flex level multiplies the time by about 2.2, which looks like a flex
container laying out its items again for each measuring pass, with no
cached result. Giving the levels `height: 100%` instead of `flex-grow`,
or making the scroller absolutely positioned inside the last flex level,
does not change the growth. A single `white-space: pre` element holding
the 100 lines costs the same as 100 rows, and 1,500 lines in one element
did not finish in ten minutes.

Measured on Windows 11, `TestApp` (headless), dependencies at
`opt-level` 2; the release window app was not measured.

Area: engine, layout (flex). Severity: blocked the diff view; any deep
flex layout with many descendants is affected.

Workaround: the window frame is absolutely positioned blocks (sidebar,
main area, status bar, tabs root, tab panels, the Changes panes), with
fixed offsets for the toolbar (48px), the tab list (12px + 36px + 8px) and
the sidebar header (81px). Flex is used only inside rows, the toolbar and
the status bar. The diff then takes about 2.6 s for 1,500 lines (gap 8).
Marked `div-gap: 26`.

## 27. A textarea ignores its padding and greys no placeholder

Tried:

```rust
view! {
    <textarea placeholder="Commit message" style:padding="8px" style:box-sizing="border-box"
        style:width="300px" style:height="80px"></textarea>
}
```

Happened (headless `TestApp` and the native window on Windows): the text
starts at the textarea's border edge; the 8px padding moves nothing,
although `computed_style().padding_left` is 8px. The placeholder is drawn
in the text color; Chromium draws it grey.
Typing, `on:input`, `control_value` and `set_control_value` work.

Area: engine, form controls (textarea text box and `::placeholder`).

Workaround (`ui/changes.rs`): the border and padding are on a box around a
borderless textarea that fills it, and "Commit message" is a muted label
over the textarea, shown while the message is empty, with
`pointer-events: none`. A click on the box's padding focuses the textarea.
Marked `div-gap: 27`.

## 28. `DynChild` wraps its content in a `<span>`

Tried: the History tab's commit pane switches between a note and the
commit's details with `DynChild(move || match state.details.get() { ... })`,
inside an absolutely positioned, scrolling `#commit-pane`.

Happened: `DynChild` returns a `<span>` that holds the current view. The
span is an inline box, so block children (the details, the diff) are
laid out inside an inline formatting context. In React, Solid and Leptos
a dynamic child adds no element of its own. (`Show` uses a container with
`display: contents`; `DynChild` does not.)

Area: framework, reactive views (`DynChild`).

Workaround (`ui/history.rs`): the span returned by `DynChild` gets
`display: contents`, so its children lay out as children of the pane.
