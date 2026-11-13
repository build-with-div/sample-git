# Framework gaps

Places where Div (pinned in `Cargo.toml`) could not do what this app needs,
found while building it. Each entry says what was tried, what happened, which
part of Div it likely belongs to, how bad it is, and the workaround used here.
Workarounds in the code are marked with a `// Div gap:` comment naming the
entry.

Areas: **engine** (style, layout, paint, text), **framework** (divui,
platform, macros, CLI), **components** (div-components).

The gaps checked before implementation are listed in the
[design](design.md#capability-gaps). Entries are added here as the
implementation reaches them.
