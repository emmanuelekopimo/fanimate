# CLAUDE.md

Guidance for Claude (and any AI assistant) working in this repository. Read it fully before changing anything. When a rule here conflicts with a habit, the rule wins.

## What Fanimate is

Fanimate is a free, open-source 2D character animation app. It keeps what animators love about **Moho Pro** (bone rigging, Smart Bones, vector puppets) and Moho Pro's screen layout, fixes the problems that push people away from Moho, and adds the best tools of **Toon Boom Harmony** and **Adobe Animate**. One Rust codebase, GPU-rendered with wgpu, native on Windows, macOS and Linux, with a browser build later.

**One-line pitch:** Moho's rigging, Harmony's drawing and pipeline, Animate's ease of use. Free, on every platform, with nothing locked away.

**Where we are:** Phase 1 (Foundation). M1 (engine spike) is done on Windows, waiting only for macOS and Linux benchmark numbers on the [reference hardware](docs/reference-hardware.md). M2 (Draw) is built: drawing, point and pen tools, booleans, point keyframes, timeline key editing, per-layer undo history and crash recovery are in; the usability pass with an artist and the exit check remain. Status lives in the "Status" section of [docs/roadmap/phase-1.md](docs/roadmap/phase-1.md). Update that section when you finish milestone work.

### Read these before non-trivial work

| Document | Why |
|---|---|
| [docs/product-vision.md](docs/product-vision.md) | The why: pain points P1 to P24, principles, feature specs. Every feature should trace back to a pain point. |
| [docs/roadmap/phase-1.md](docs/roadmap/phase-1.md) | What is in and out of scope now, workstreams, milestones, Gate 1 tests, current status |
| [docs/spec/fnm.md](docs/spec/fnm.md) | The project file format. Must match `crates/fanimate-fnm` exactly. |
| [brand/README.md](brand/README.md) | Colours, themes, icon and wordmark rules |
| [CONTRIBUTING.md](CONTRIBUTING.md), [GOVERNANCE.md](GOVERNANCE.md) | Workflow, review standards, release process |

## Non-negotiable rules

1. **Clean room.** Never copy, trace or derive code, icons, artwork or text from Moho, Adobe Animate, Toon Boom Harmony or any closed product. Never use decompiled or leaked material. Follow Moho's *layout and workflow*, never its *look or code*. Importers are written from public documentation and from inspecting files we may use. Screenshots of other apps in `reference/` are for layout reference only and are gitignored: **never commit anything under `reference/`.**
2. **Never lose work.** Saving is atomic (write a temp folder, swap it in). Loading validates and repairs. No panics on user data: bad input becomes a validator issue or an error message. Anything touching save, load or undo needs tests, including the failure path.
3. **Fast by default.** The Gate 1 budget is 60 fps for a 40-bone rig with 10 Smart Bones on a mid-range laptop, project open under 3 s, no UI action over 100 ms. Per-frame code (evaluation, tessellation, rendering) must not allocate needlessly; give before and after numbers when you change it.
4. **Polish before features.** A smaller finished feature beats a larger half-finished one. Unfinished tools are shown but clearly marked with the milestone that brings them; they never pretend to work.
5. **The file format is a public contract.** Any change to what `.fnm` writes needs: an update to `docs/spec/fnm.md`, a version bump if older readers would misread it, a migration on load, validator coverage, and a round-trip test. Files from any released version must always open.
6. **Ask before outward-facing actions.** Do not commit, push, tag, create releases or change GitHub settings unless the user asks. Releases go through `pnpm release` only.

## UI and UX rules

Fanimate looks like Moho Pro in **layout** (tools left of the canvas, timeline along the bottom, Style/Actions and Layers on the right, tool names and concepts like bones, Smart Bones, actions, switch layers) but must not copy Moho's **interaction problems**. Users find Moho's layer handling unusual and hard to use. So:

- **Right-click menus everywhere.** Every list row, canvas, timeline row and key area has a context menu. Right-clicking an unselected item selects it first. Open them with `menus::context_menu` (not egui's `Response::context_menu`) so they share the menu bar's style, and fill them with the same `menus::item` helpers, so labels, shortcuts and enabled states always match.
- **Feel native.** Labels are not selectable and show the arrow cursor; text shows an I-beam only where it can be edited. Do not show grab or move cursors on tabs and list rows (`native_cursors` in `app.rs` removes egui's). An edit field asks for focus once when it opens, never every frame, so clicking elsewhere commits it.
- **Standard shortcuts by default, Moho shortcuts on request.** Two keymaps ship (`crates/fanimate-editor/src/keymap.rs`):
  - **Fanimate (default):** Ctrl+C / Ctrl+X / Ctrl+V copy, cut and paste whatever is selected *including layers*; Ctrl+Z undo; **Ctrl+Y and Ctrl+Shift+Z redo**; Delete deletes; F2 renames; Ctrl+D duplicates; tool letters follow Adobe Animate and Illustrator.
  - **Moho:** Moho Pro's own shortcuts (Ctrl+Shift+Z redo only, M transform layer, Z manipulate bones, T transform points, A add point or bone depending on layer type, Home resets the view).
  Users pick one in the first-run welcome dialog and can switch any time (Edit > Keyboard Shortcuts, Preferences, or the status bar). Never hard-code a shortcut in UI code: add the `Action`, bind it in **both** keymap tables (or deliberately leave it unbound in Moho), and let menus show it via `Keymap::shortcut_text`. The keymap test rejects duplicate chords.
- **Menus live in the title bar.** The window is undecorated on Windows and Linux; `titlebar.rs` draws the menus, the centred document title, window buttons, dragging, double-click to maximise and edge resizing. The menu bar behaves like a desktop one: once a menu is open, hovering another title or pressing Left/Right switches to it, and shortcuts are ignored while any menu is open. On macOS the native traffic lights stay. Do not add a separate menu row.
- **No modal dialogs for properties.** Layer and project settings are edited live in the docked Properties panel (Moho uses blocking dialogs). Reserve modals for decisions (unsaved changes, first run) and the command palette.
- **Standard selection and editing:** click, Ctrl-click, Shift-click ranges; drag and drop to reorder or nest layers; double-click or F2 to rename in place; one click on the eye to show or hide.
- **Every command is an `Action`.** Menus, shortcuts, context menus and the command palette all queue the same `Action` and run through `Editor::run`. Never put editing logic in a widget.
- **One gesture, one undo step.** Call `Editor::checkpoint` once per user action (at drag start, or via `properties::begin_edit` for value widgets), never per frame.
- **Auto-key rule:** editing an animated channel sets a key on the current frame; editing an unanimated channel changes its value for the whole animation.

### Writing UI text

- **Never use em dashes (—) in any UI text**: labels, menus, tooltips, status messages, dialogs, window titles, CLI output. Use a colon, comma, full stop or parentheses instead, or a plain hyphen in window titles ("Scene - Fanimate"). A test in `fanimate-app` fails the build if an em dash appears in Rust source.
- Plain, short, sentence case. Name things the way animators do. Say what will happen ("Delete 3 layers"), not how the code works.
- Use the ellipsis character "…" only on menu items that open a dialog or need more input.
- Status messages are brief and disappear after a few seconds; errors say what went wrong and what to do.

### Brand and theming

- Colours come only from `brand/tokens/tokens.json`. `crates/fanimate-app/src/theme.rs` reads that file at compile time. Do not hard-code colours in UI code; use `shell.palette`.
- **Purple marks interaction, not content:** selection, focus, active tools, primary buttons. Animation state has its own colours: keyframes and the playhead amber (`keyframe`), bones teal (`bone`), errors coral.
- The canvas pasteboard stays neutral; never tint artwork.
- **Layer label colours** (`labels` in the tokens, `fanimate_core::Label`) mark layers in the Layers panel and timeline rows, like compositing apps' label colours. New layers take the next label in turn (`Label::for_id`). They never colour artwork, keyframes or selection.
- Studio Grey is the default theme.
- The Bone-F mark always has its see-through pivot dot. Brand assets are generated by `brand/scripts/build_brand.py`; edit the script or tokens, never the generated files.
- UI font is Inter (bundled, OFL). Keep text legible at 100% scaling on a 1366x768 laptop, the smallest screen we design for.
- **Interface icons are [Lucide](https://lucide.dev) SVGs** (ISC licence) in `crates/fanimate-app/icons/lucide/`, rasterised with resvg by `crates/fanimate-app/src/icons.rs`; never font glyphs or emoji (egui's fonts lack many symbols and they render as empty boxes). To add one, download the SVG from the same lucide-static version (see `icons/lucide/README.md`), add a variant to `Icon`, map it in `Icon::svg`, and list it in the icon test. Icons Lucide lacks (bone tools) go in `icons/fanimate/`, drawn on Lucide's 24 px grid with 2 px round strokes. Simple shapes that are not icons, like keyframe diamonds, are painted directly.

## Architecture

```
crates/
  fanimate-core     Scene graph, animation channels, bones, evaluation. No UI, GPU or file IO.
  fanimate-fnm      .fnm reader, atomic writer, validator and repair.
  fanimate-render   lyon tessellation + wgpu renderer; headless render to image.
  fanimate-editor   Editing model: Editor (selection, commands, undo), Action, Tool, keymaps, clipboard. No egui.
  fanimate-app      The desktop app (eframe/egui + egui_dock): title bar, panels, canvas, dialogs.
  fanimate-cli      Headless tool: sample, validate, render, bench.
```

Dependencies point downwards only: core ← fnm ← editor ← app; core ← render ← app/cli. **Keep `fanimate-core` free of UI, GPU and platform code** (it runs headless and in the browser), and **keep `fanimate-editor` free of egui** so every command is unit-tested without a window. If logic can live in `fanimate-editor`, it belongs there, with a test.

Key facts:

- **Crash recovery depends on `Editor::revision`.** It goes up on every `checkpoint`, undo and redo, and the app journals the document once the revision changes and no gesture is in progress (`fanimate-app/src/recovery.rs`, `fanimate_fnm::journal`). An edit that changes the document without a checkpoint is only journalled by the 10-second backstop, so always checkpoint.
- A brush stroke is an ordinary shape: its path is the centreline and `Shape::widths` the width profile (one value per on-curve point). Anything that adds or removes points must keep `widths` **and `path_keys`** in step (`Shape::remove_points`, `insert_point` and `extend_from` do). Every point key has the same commands as the shape's path; edit points through `Shape::path_at` / `set_path_at` so the auto-key rule holds.
- **Undo history is shared per top-level layer** (`fanimate_fnm::HistoryState`): a state reuses the `Arc` of every layer unchanged since the previous one. Keep it that way (capture with `HistoryState::capture` and a previous state), or the journal rewrites every layer on every step. Code that needs a shape's index uses `DrawItem::index`, never pointer comparison: a shape with point keys is drawn from a per-frame copy.

- Document space has **y pointing down**, angles in **radians**, positive is clockwise on screen. Frames are integers; frame 0 is the first; negative keyframes are invalid.
- Layers are drawn first to last (last is on top). The Layers and Timeline panels list them **top of stack first** (`Editor::display_rows`).
- A channel's `base` is its value when unkeyed and, for bones, the rest pose.
- The canvas renders through `fanimate-render` into a texture registered with egui (`canvas_gpu.rs`): render through an `Rgba8UnormSrgb` view, sample as `Rgba8Unorm`. Canvas and export share the renderer, so the screen matches the output.
- **wgpu, winit and egui versions are coupled.** wgpu 30 / winit 0.30 / eframe and egui 0.36 / egui_dock 0.21. Upgrade them together and check `cargo tree -d` shows a single wgpu.
- **Windows uses DirectX 12 by default**, never Vulkan, and OpenGL only when DirectX 12 has no hardware adapter: the Vulkan driver for some Intel GPUs (seen on HD Graphics 520) crashes the process on load, uncatchably, and the same machine's OpenGL driver crashes in about 1 of 40 runs at process exit even when merely loaded to list adapters. `fanimate_render::backends()` is the single source of this choice; `WGPU_BACKEND` overrides it.
- egui turns Ctrl+C/X/V into `Event::Copy`/`Cut`/`Paste`, not key events; `app.rs` handles them directly. Shortcuts are ignored while a text field has focus, a modal or a menu is open.
- **On Windows the app is a GUI program** (`windows_subsystem = "windows"`), so no console window opens. Print user-facing output with `crate::report`, which falls back to a message box when there is no console.
- **A maximised borderless window must not cover the whole monitor.** winit sizes it to the work area, which is the whole monitor when the taskbar auto-hides; Windows then treats it as fullscreen (the taskbar stops appearing, Alt+Tab previews and OBS capture can go blank). `titlebar::track_window` replaces that maximise with a normal window one point short of the bottom edge. Keep it when changing window code.

## Commands

```sh
cargo run --release -p fanimate-app                 # run the app
cargo run --release -p fanimate-app -- --bench      # 10 s playback benchmark
cargo run --release -p fanimate-cli -- bench        # CPU benchmark
cargo run --release -p fanimate-cli -- bench --gpu  # plus offscreen GPU render, no window
cargo run --release -p fanimate-cli -- render scene.fnm -o out.png --frame 12

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p fanimate-core -p fanimate-fnm -p fanimate-editor -p fanimate-render --target wasm32-unknown-unknown

python brand/scripts/build_brand.py           # regenerate brand assets
python brand/scripts/build_brand.py --check   # contrast + themes.css check

pnpm release --dry-run                        # what a release would do
```

**CI does not run on every push.** `pnpm release` (scripts/release.js) runs fmt, clippy, tests and the brand check locally, bumps the version in `Cargo.toml`, commits, tags `vX.Y.Z` and pushes; the tag triggers `.github/workflows/release.yml`, which builds and tests on Windows, macOS and Linux and drafts a GitHub release. So **run the checks above yourself before saying work is done.**

## Code conventions

- Rust edition 2024, `rustfmt.toml` (max width 110). `cargo clippy -D warnings` must pass. `unsafe` is denied workspace-wide; do not add it.
- **The Rust version is pinned in `rust-toolchain.toml`** and the release workflow installs exactly that version. Never switch CI back to "latest stable": new Rust releases add clippy lints, and an unpinned CI fails on code that passed locally. To upgrade, bump the pin, run clippy and the tests, fix, and commit in one change.
- Match the surrounding code: comment density, naming and idioms. Comments explain **why**, not what. Module docs say what the module is for.
- No placeholder code: no `let _ = SomeType::X;` to silence imports, no dead branches, no `todo!()` in shipped paths. Remove unused imports instead.
- Errors: libraries return typed errors (`thiserror`); the app shows them in the status bar or a dialog. Never `unwrap` on user data or IO.
- Tests: every editor command, file format change and bug fix gets a test. A bug fix's test must fail before the fix. GPU tests skip cleanly when no adapter exists.
- Dependencies must be MIT/Apache-compatible (no GPL/LGPL in the app or engine). Check versions with `cargo search`; prefer versions at least two weeks old.
- Keep `Cargo.lock` committed. Keep generated assets in sync with their sources.

## Documentation

- Update docs in the same change as the code: the spec for format changes, the Phase 1 status for milestone work, CONTRIBUTING for workflow changes, this file for new rules.
- Docs are plain Markdown for a technical audience. Avoid marketing language. Distinguish what exists from what is planned.

## Things not to do

- Do not add features from later phases (node graph, scripting, marketplace, cloud, AI) during Phase 1 without the user asking; record ideas in the roadmap instead.
- Do not make the app depend on network access. Everything works offline.
- Do not add telemetry that is on by default.
- Do not reintroduce Moho's problems: modal settings dialogs, missing context menus, layer operations without shortcuts, or a Moho-only shortcut scheme.
