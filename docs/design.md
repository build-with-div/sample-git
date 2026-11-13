# Sample Git: design

A small local git client built on [Div](https://github.com/build-with-div/div),
using only Div's public API. It has two jobs:

1. Be a useful, small git GUI for someone with many repositories on disk.
2. Dogfood Div. Each place where Div cannot do what the app needs is
   recorded in [framework-gaps.md](framework-gaps.md) and reported to the
   Div tracks rather than patched here.

This document is the v1 design. Everything in the
[capability gaps](#capability-gaps) section was checked against Div
`03573e7d` (build-with-div/div `main`, 12 November 2026), by reading the
source and by running headless probes with `divui`'s `testing` feature on
Windows. The probe results are quoted where they decide a workaround.

## Users and scope

The user keeps many clones on disk (work, side projects, dotfiles) and
mostly uses git from a terminal. They want to:

- see at a glance which repositories have uncommitted changes, which branch
  each is on, and which are ahead of or behind their upstream;
- review a diff, stage files and commit without opening a terminal;
- browse recent history and switch branches;
- fetch, pull and push, and see progress and errors.

It is not a full git client. v1 leaves out merge-conflict resolution,
rebase, stash, blame, submodule management, hunk or line staging, and force
push. Those stay in the terminal.

## v1 features

| Area | v1 | Later |
| --- | --- | --- |
| Repository list | Add a repository (folder picker). Scan a folder for repositories (to a depth of 3, skipping `node_modules`, `target` and hidden folders) and add the ones found. Remove from the list, which never touches the disk. Each row shows the name, branch, a change count and ahead/behind. The list is saved between runs. | Groups, filter box, sort by activity |
| Changes | Staged and unstaged lists (untracked files are unstaged). Stage or unstage one file or all. Discard changes to one file, with a confirmation. | Hunk staging, discard all |
| Diff viewer | Unified diff of the selected file, staged or unstaged: old and new line numbers, colored added and removed lines, hunk headers. Binary files show a notice; diffs over 1,500 lines show the first 1,500 and a "Show full diff" button. | Side-by-side view, word diff |
| Commit | Message box, Commit button (disabled with nothing staged or an empty message), Ctrl+Enter. Hooks and signing run, because the commit goes through `git commit`. | Amend, co-author templates |
| History | The last 200 commits of HEAD with "Load more". Each row: short hash, subject, author, relative date. Selecting one shows its full message, author, date, parents and changed files; selecting a file shows that commit's diff of it. | Graph, search, other refs |
| Branches | Local branches with the current one marked, upstream and ahead/behind. Switch to a branch. Create a branch from HEAD. Delete a merged branch (`git branch -d`), with a confirmation. Remote branches listed; checking one out creates a tracking branch. | Rename, merge, delete unmerged |
| Remote | Fetch, Pull (`--ff-only`) and Push for the current branch, with progress in the status bar, a Cancel button, and errors shown in full. Push of a branch without an upstream asks before running `git push -u origin <branch>`. | Choose remote, tags |

Out of v1 on purpose: force push (no UI at all), deleting unmerged
branches, discarding all changes at once. Every action that can lose work
asks first: discarding a file's changes and deleting a branch.

## Screen layout

One window, minimum 960 × 600, three columns under a toolbar, with a status
bar at the bottom. The menu bar is native on Windows and drawn by Div on
Linux.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ File  Repository  View  Help                                             │
├────────────────┬─────────────────────────────────────────────────────────┤
│ REPOSITORIES   │ sample-app   main  ↑2 ↓0         [Fetch] [Pull] [Push]  │
│ [Add] [Scan]   ├─────────────────────────────────────────────────────────┤
│                │ [Changes 3] [History] [Branches]                        │
│ ▌sample-app    ├────────────────────────┬────────────────────────────────┤
│ ▌main · 3      │ Staged (1)  [Unstage all] src/lib.rs (staged)          │
│  notes         │  M src/lib.rs      [−]  │ @@ -1,4 +1,5 @@               │
│  dev · clean   │ Changes (2)  [Stage all]│  1  1  use std::io;           │
│  dotfiles      │  M README.md   [+] [⟲]  │     2 +use std::fs;           │
│  main · ↓3     │  ? todo.txt    [+]      │  2  3  fn main() {            │
│                │                         │                               │
│                │ Commit message          │                               │
│                │ ┌─────────────────────┐ │                               │
│                │ │                     │ │                               │
│                │ └─────────────────────┘ │                               │
│                │ [Commit to main]        │                               │
├────────────────┴────────────────────────┴────────────────────────────────┤
│ Fetching origin: Receiving objects 45% (912/2026)               [Cancel] │
└──────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar** (240 px): repository list. The selected row has an accent bar.
- **Toolbar**: repository name, branch, ahead/behind, remote buttons.
- **Tabs**: Changes, History, Branches. Changes shows its change count.
- **Left pane** (320 px): the tab's list. Changes adds the commit box under
  the lists. History lists commits. Branches lists branches and has a
  "New branch" input.
- **Right pane** (the rest): the diff viewer for Changes, commit details
  plus the diff of the selected file for History, empty for Branches.
- **Status bar**: the running or last operation, its progress, Cancel, and
  an error banner that expands to the full git output.
- **Confirmation dialog**: a scrim over the window with a card in the middle
  ("Discard changes to README.md? This cannot be undone." with Cancel and a
  destructive button). Focus is kept inside the card while it is open.

Pane widths are fixed in v1 (see [split panes](#capability-gaps)).

### Menus and shortcuts

| Menu | Items |
| --- | --- |
| File | Add Repository… (Ctrl+O), Scan Folder…, Remove from List, Quit |
| Repository | Refresh (F5), Fetch, Pull, Push, Copy Path |
| View | Changes (Ctrl+1), History (Ctrl+2), Branches (Ctrl+3) |
| Help | About |

In content: Ctrl+Enter commits from the message box, Escape closes the
confirmation dialog, and the arrow keys move the selection in the focused
list.

## Git backend: the `git` CLI

The app runs the user's installed `git` (2.30 or later) as a child process.
It does not link `git2` (libgit2) or `gix`.

Why:

- **Same behavior as the user's terminal.** Credential helpers (Git
  Credential Manager on Windows), SSH agents and keys, `includeIf`, hooks,
  commit signing, LFS filters, `core.autocrlf` and `safe.directory` all apply
  without the app knowing about them. A git GUI that commits without running
  the repository's hooks, or pushes without the user's credential helper, is
  wrong in ways that are hard to see.
- **Push works.** `gix` has no complete push yet. `git2` pushes but needs its
  own credential callbacks and SSH transport (libssh2), and builds OpenSSL
  or libssh2 on Linux, adding to a build that already compiles Skia.
- **Stable machine formats.** `git status --porcelain=v2 -z --branch`,
  `git log -z --format=…`, `git for-each-ref --format=…` and
  `git diff --no-color --no-ext-diff` are documented for scripts.
- **No unsafe code or native dependencies** of the app's own.

Costs: git must be installed (checked at start; the app shows a message if
it is missing or too old), each command costs a process start (tens of
milliseconds on Windows), and output must be parsed. Parsers are pure
functions with unit tests.

### How commands run

- Always `git -C <repo> <args…>` built as an argument vector, never through
  a shell. Paths from the user are passed after `--`.
- Environment for every command: `GIT_TERMINAL_PROMPT=0` (fail rather than
  wait for a terminal that does not exist), `LC_ALL=C` and `LANGUAGE=` (for
  parseable progress and stable messages), and `GIT_OPTIONAL_LOCKS=0` for
  read-only refreshes so a background status does not block the user's own
  git commands. stdin is null.
- Commands never block the UI thread. Each runs with
  `div::task::spawn_blocking`; the UI awaits it in a `spawn_local` task and
  writes the result into signals. Fetch, pull and push read stderr line by
  line on the blocking thread (`--progress`) and post progress to the UI
  with `Signal::sender`. Cancel kills the child process.
- One mutating command per repository at a time. A per-repository busy flag
  disables the buttons while one runs; read-only refreshes can overlap.
- Status for every listed repository refreshes at start (at most four
  at a time), after each operation, from Refresh (F5), and every 30 s
  for the selected repository.

### Module layout

```text
src/
  main.rs            App, window and menu wiring
  lib.rs             re-exports for the tests
  git/               no Div dependency; unit-tested on its own
    command.rs       runner: env, args, output, cancel, progress lines
    status.rs        porcelain v2 parser → branch, upstream, ahead/behind, entries
    diff.rs          unified diff parser → files, hunks, numbered lines
    log.rs           log and commit-detail parser
    branch.rs        for-each-ref parser, switch, create, delete
    remote.rs        fetch, pull, push and the progress-line parser
    scan.rs          folder scan for repositories
  store.rs           saved repository list (a text file in the user's config dir)
  state.rs           app state: signals per repository and per view
  ui/
    sidebar.rs  toolbar.rs  changes.rs  diff_view.rs  history.rs
    branches.rs  status_bar.rs  confirm.rs  style.rs (shared style helpers)
tests/
  git_*.rs           git layer against fixture repositories
  ui_*.rs            headless UI tests (TestApp) against fixture repositories
```

The repository list is stored in `%APPDATA%\div-sample-git\repos.txt` on
Windows and `$XDG_CONFIG_HOME/div-sample-git/repos.txt` (or
`~/.config/…`) on Linux, one path per line. Tests point it elsewhere.

## Component inventory

What each piece is built from in Div today.

| UI piece | Built from |
| --- | --- |
| Layout (columns, toolbar, status bar) | `view!` with inline `style:` flex properties |
| Repository, file, commit and branch rows | `For` keyed by path, hash or ref name, inside a small `Slot` component (see gaps) |
| Selected and hovered rows | Dynamic `style:background-color={…}` (a typed `Color`) from selection and hover signals |
| Buttons | `div_components::button::Button` (default, outline, ghost, destructive), wrapped for clicks (see gaps) |
| Status letters, counts, ahead/behind | `div_components::badge::Badge` |
| Changes / History / Branches | `div_components::tabs` (`Tabs`, `TabsList`, `TabsTrigger`, `TabsContent`) |
| Pane dividers | `div_components::separator::Separator` |
| Commit details | `div_components::card` |
| Commit message | native `<textarea>`, `Element::control_value` / `set_control_value` |
| New-branch name | native `<input type="text">` |
| Diff text | `Geist Mono` from `div_components::fonts::register_fonts`, `white-space: pre` |
| Empty and loading states | `Show` |
| Menu bar and shortcuts | `div::menu` (`Menu`, `MenuItem`, `Accelerator`, roles) |
| Add / Scan | `div::dialog::pick_folder()` |
| Confirmation dialog | absolutely positioned scrim and card, `Document::set_modal_root` |
| Copy hash or path | `div::clipboard::write_text` |
| Background git | `div::task::{spawn_blocking, spawn_local}`, `Signal::sender` |
| Text and theme | `div_components::theme` tokens (light), Geist |

## Testing

- **Parsers**: unit tests over captured git output (status, diff, log,
  for-each-ref, progress lines), including renames, paths with spaces and
  non-ASCII, binary files and detached HEAD.
- **Git layer**: integration tests that build scratch repositories and a
  local bare "origin" and run the real `git`: stage, unstage, discard,
  commit, branch create/switch/delete, fetch, pull and push over a file URL.
  No network.
- **UI**: headless `div::testing::TestApp` tests mount the app against
  scratch repositories: select a repository, click a file and see its diff,
  stage it, type a message, commit, see the commit in History, switch
  branches, and check that discard asks first and Cancel keeps the file.
- **Isolation and safety**: tests only create and touch repositories under
  a fixtures root: `SAMPLE_GIT_FIXTURES` if set (locally
  `E:\open-ui\out\sample-git-fixtures`), else a folder under the system temp
  directory. They set `GIT_CONFIG_GLOBAL` to a file in that root and
  `GIT_CONFIG_NOSYSTEM=1`, so the developer's own git config, hooks and
  credentials are never used, and the fixture helper refuses any path
  outside the root. Development runs of the app use only fixture
  repositories too.
- **CI**: build, `cargo fmt --check`, clippy and tests on `windows-2025` and
  `ubuntu-24.04`, with the Skia build variables of Div's CI. UI tests that
  compare pixels are left out of v1, because text rendering differs between
  the platforms and from run to run of the toolchain; UI tests assert on
  the document instead.

## Capability gaps

Checked before writing app code. "v1" means the design needs it;
"nice" means v1 ships without it. Each confirmed gap gets an entry in
[framework-gaps.md](framework-gaps.md) with a minimal snippet when the
implementation reaches it.

The probe is kept in [probes/](probes/) (`capability_probe.rs` and the
`Cargo.toml` it ran with; copy both into an empty crate's `tests/` and root,
then `cargo test -- --nocapture --test-threads=1`). Its numbers come from a
debug build with dependencies at `opt-level = 2` (as the CLI template sets),
headless, on Windows 11, one run each: indicative, not benchmarks.

| # | Capability | Need | Div `03573e7d` | Workaround in v1 | Area |
| --- | --- | --- | --- | --- | --- |
| 1 | Hover, focus and active styling | nice | No `:hover`/`:focus`/`:active` (engine drafts #16–#19). Probe: `mouseenter`/`mouseleave` fire and a dynamic `style:` repaints when its signal changes. | One `hovered` signal per list drives a dynamic background. Focus is shown by the native outline only. | engine: style |
| 2 | Shared styles (classes, a stylesheet) | nice | Inline `style:` only; no cascade or class selectors. | Style helper functions in `ui/style.rs` (`style!` blocks applied with `Element::apply_style`), repeated inline styles elsewhere. | engine: style |
| 3 | Dynamic style values | v1 | `style:x={expr}` needs the typed value (`Color::from_rgba8`, `LengthValue::px`); a CSS string compiles only as a static literal. | Typed constructors. Ergonomic, not blocking. | framework: macros |
| 4 | Common style literals | v1 | Rejected at compile time: `inset`, the `flex` shorthand, `cursor: col-resize`. The components track also lists `box-shadow`, `border: … currentcolor` and others. | Longhands (`top`/`right`/`bottom`/`left`, `flex-grow`); no resize cursor. | engine: style |
| 5 | Click handler on a component | v1 | `<Button on:click=…>` compiles, and the handler is silently dropped (`view!` ignores `on:` on component tags); `Button` has no click prop. Probe: direct handler never ran; a wrapper `<span on:click>` received the bubbled click. | Wrap each `Button` in an element that owns the handler. | framework: macros; components |
| 6 | `For`/`Show` inside `view!` markup | v1 | A `{expr}` child is always converted to text, so `{For(…)}` cannot be a child. | A three-line `Slot` component (`fn Slot(view: ViewNode) -> ViewNode`). Probe: keyed `For` and `Show` update correctly inside it. | framework: macros |
| 7 | Optional component props | v1 | Omitted `#[prop(optional)]` props are not filled (RFC 0003 (c) in progress). | Pass every prop. | framework: macros |
| 8 | Long lists (history, large diffs) | v1 for 10³ rows; virtualization nice | No virtualized list. Probe: `For` over 2,000 rows takes ~600 ms to mount and ~480 ms to replace; appending one row to 2,000 takes ~41 ms; rendering is ~4 ms. | History pages of 200 commits with "Load more". Diffs show the first 1,500 lines with "Show full diff". | framework (list component) or components |
| 9 | Multi-line text input (commit message) | v1 | Works. Probe: `<textarea>` takes focus on click, typing and Enter produce `"fix: bug\nbody"`, `on:input` fires, `set_control_value("")` clears it. | None. | — |
| 10 | Keyboard shortcuts | v1 | Menu accelerators exist. In content, `on:keydown` gives `key_code`, `key_text` and `modifiers` (probe: Ctrl+Enter = code 13, mods CTRL). Named keys (`Event::key()`) are in RFC 0003 (b), not merged. | Match key codes; move to `Event::key()` when it lands. | framework |
| 11 | Monospace diff text | v1 | Works. `register_fonts` adds Geist Mono; ten `i` and ten `W` are both 72 px at 12 px. `white-space: pre` keeps spaces. | None. | — |
| 12 | Truncating long paths | v1 | Works: `text-overflow: ellipsis` with `overflow: hidden; white-space: nowrap`. | None. | — |
| 13 | Scrolling panes | v1 | Works: `overflow-y: auto`, wheel (300 px per probe event), `scroll_to`, `scroll_metrics`. | None. | — |
| 14 | Confirmation dialogs | v1 | No top layer, `<dialog>` or native message box. Probe: an absolutely positioned scrim with `z-index` blocks clicks to what it covers, and `Document::set_modal_root` limits input to the card. | In-window overlay (`ui/confirm.rs`) with Escape to cancel. | framework (dialog/top layer); engine (top layer) |
| 15 | Context menus | nice | No popup-menu API. Probe: a right click (down and up) fires no `contextmenu` event. | Row buttons (stage, unstage, discard) instead of a menu. | framework: platform/menu, events |
| 16 | Resizable split panes | nice | `set_pointer_capture` exists, but probe input from `dispatch_mouse_event` produces `mousemove`/`mousedown` and no `pointer*` events, and moves after the press outside the handle are not delivered to it. Native input not yet checked. | Fixed pane widths. | framework: events |
| 17 | Icons | nice | Lucide icons cannot be drawn yet (static SVG paints `<rect>` only). | Text labels and a few Unicode glyphs (↑ ↓ ●). | engine: paint |
| 18 | `<select>` | not used | Probe: initial `control_value()` is `""` (Chromium gives the first option), and a click changes the value to the second option without a popup. Reported as a bug; v1 uses lists instead. | Branch switching from the Branches list. | engine/framework: forms |
| 19 | Dark mode | nice | No `prefers-color-scheme`, media queries or custom properties. | Light theme only. | engine: style |
| 20 | App data directory, open in file manager | nice | No API for either. | `%APPDATA%` / `$XDG_CONFIG_HOME` read directly; "Copy Path" instead of "Open in Explorer". | framework: platform |
| 21 | Window activation event (refresh on focus) | nice | None found in the public API. | Refresh after each operation, on F5, and every 30 s. | framework: platform |
| 22 | Tooltips | nice | None found in the public API. | Full paths shown in the diff header. | framework |

Not yet checked and left to implementation: text selection and copy in
the diff view, and how fast a 200-row `For` updates when one status
changes. Rows 15, 16 and 21 should be rechecked with native input on
Windows.

So v1 is not blocked. It depends on seven workarounds (rows 3–8 and 14), each
small; the ones most worth fixing in Div for this app are 5 (dropped
component handlers, a silent failure), 6 (`For` inside `view!`), 8 (list
cost) and 14 (a dialog primitive).

## Delivery plan

Small PRs, each green on CI before it is merged:

1. Design (this document) and the gap list.
2. Skeleton: Cargo manifest pinned to a Div commit, CI on both platforms,
   an empty window, the fixture helper.
3. Git layer: runner and parsers with unit and fixture tests.
4. Repository list: add, scan, remove, saved list, status refresh.
5. Changes: lists, stage and unstage, diff viewer.
6. Commit box and discard with confirmation.
7. History and commit details.
8. Branches: list, switch, create, delete.
9. Fetch, pull and push with progress and cancel.
10. README with screenshots and build instructions; gap list brought up to
    date.
