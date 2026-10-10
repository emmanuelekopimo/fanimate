# Fanimate Phase 1: Foundation

Companion to the [product vision](../product-vision.md). The brand assets proposed in section 1 live in [`brand/`](../../brand/). Phase 1 runs about 12 months and ends at **Gate 1: version 1.0**. This document starts with the brand, because every screen, installer and website in Phase 1 depends on it, and then breaks the engineering work into workstreams, milestones and acceptance tests.

> **What comes from the vision and what is new.** The scope, the Gate 1 criteria and the performance budget are taken from the product vision. The brand proposals, the milestone order, the task lists and the acceptance tests are new proposals, written so the team can challenge and change them.

## Contents

1. [Branding assets](#1-branding-assets)
2. [Phase 1 scope](#2-phase-1-scope)
3. [Workstreams](#3-workstreams)
4. [Milestones](#4-milestones)
5. [Gate 1 acceptance tests](#5-gate-1-acceptance-tests)
6. [Phase 1 risks](#6-phase-1-risks)
7. [First 30 days](#7-first-30-days)

---

## 1. Branding assets

### 1.1 Brand direction

**Purple is the brand colour.** It suits Fanimate for three reasons:

- It reads as creative and imaginative, and is not tied to the red of Adobe or the blue-greys of most developer tools.
- It sits well on dark backgrounds, which is where animators work for hours.
- It leaves room for warm accent colours (amber, coral) that mark animation state such as keyframes and the playhead.

**Personality:** friendly, capable, a little playful. It should feel like a tool made *by* animators, not an enterprise suite.

**Taglines to test:**

| Tagline | Tone |
|---|---|
| *Rig it. Draw it. Move it.* | Direct; names the three strengths |
| *Animation, unlocked.* | Reflects "nothing locked away" and the free, open model |
| *Free to animate.* | Short; works for the Nigeria and education audience |
| *Every frame yours.* | Reflects open file format and ownership of work |

Recommended starting pick: **"Free to animate."** It is short and states the main difference from Moho, Harmony and Animate.

### 1.2 Colour palette

#### Brand purple scale (Fanimate Violet)

| Token | Hex | Use |
|---|---|---|
| violet-50 | `#F5F0FF` | Tints, light-theme hover |
| violet-100 | `#EBE0FF` | Light-theme selection background |
| violet-200 | `#D6C2FF` | Light-theme borders on focus |
| violet-300 | `#BC9BFF` | Dark-theme text links, selection outline |
| violet-400 | `#9F73FF` | Dark-theme interactive accent |
| **violet-500** | **`#8450F5`** | **Hero brand colour: logo, splash, marketing** |
| violet-600 | `#6D35DE` | Light-theme primary buttons |
| violet-700 | `#5826B8` | Pressed state, light-theme links |
| violet-800 | `#431D8C` | Dark-theme deep accents |
| violet-900 | `#2F1463` | Dark backgrounds on marketing pages |
| violet-950 | `#1C0B3D` | Splash screen, deepest brand surface |

#### Functional accents

Purple alone cannot carry every meaning in an animation tool, so Phase 1 reserves a small set of accent colours for animation state. Keep them consistent across themes.

| Role | Hex (dark theme) | Hex (light theme) | Where it appears |
|---|---|---|---|
| Keyframe / playhead | `#FFB84D` amber | `#9A5B00` | Keys on the timeline, playhead, current-frame marker |
| Bone overlay | `#5EEAD4` teal | `#0F766E` | Bones, IK handles, Smart Bone drivers on canvas |
| Success / saved | `#3DDC97` mint | `#157347` | Autosave confirmed, import report passed |
| Error / destructive | `#FF6B8E` coral | `#C2294F` | Rig health errors, delete confirmations |
| Onion skin: past | `#FF6B8E` coral | `#C2294F` | Previous frames |
| Onion skin: future | `#3DDC97` mint | `#157347` | Next frames |

Colour must never be the only signal. Pair every state colour with an icon or label, which also helps colour-blind users.

#### Purple-tinted neutrals

Using grey with a faint purple tint keeps the interface cohesive. Pure neutral grey next to purple looks disconnected.

| Token | Dark theme | Light theme |
|---|---|---|
| Canvas surround (pasteboard) | `#100C18` | `#EAE5F5` |
| App background | `#15111F` | `#FAF8FF` |
| Panel | `#1D1829` | `#F1EDFA` |
| Raised surface (menus, dialogs) | `#272036` | `#FFFFFF` |
| Border | `#352D47` | `#DDD5EE` |
| Text | `#EDE9F7` | `#1D1629` |
| Muted text | `#A79DC2` | `#5F5478` |
| Primary button fill | `#8450F5` (white text) | `#6D35DE` (white text) |

**Contrast checks** (WCAG, 4.5:1 is the AA minimum for normal text):

| Pair | Ratio |
|---|---|
| Dark: text on app background | 15.6:1 |
| Dark: muted text on panel | 6.8:1 |
| Dark: violet-400 on app background | 5.6:1 |
| Dark: violet-300 on panel | 7.6:1 |
| Dark: amber / mint / coral on app background | 10.8 / 10.5 / 6.8:1 |
| Light: text on app background | 16.6:1 |
| Light: muted text on panel | 6.0:1 |
| Light: violet-600 on app background | 6.2:1 |
| White on violet-600 (button) | 6.5:1 |
| White on violet-500 (button, dark theme) | 4.7:1 |

All pass AA. Re-run these checks whenever a colour changes; the validator can live in the repo as a small script so the theme never regresses.

### 1.3 Theme suggestions

Phase 1 ships **three built-in themes** plus an accessibility option. Themes are defined as design tokens (a single file of named values, such as `--panel`, `--accent`), so the UI never hard-codes a colour and the community can add themes later through the Phase 2 and 3 extension system.

| Theme | Character | Notes |
|---|---|---|
| **Fanimate Night** (default) | Deep purple-tinted dark. Violet-400 accent. | The default, because most animators work in dim rooms and dark panels stop the UI competing with artwork. |
| **Fanimate Day** | Light lavender-white. Violet-600 accent. | For bright studios and classrooms; also the theme of the website and docs. |
| **Fanimate Studio Grey** | Mid-dark neutral with a purple accent. | A grey UI for colour-critical work, where a tinted surround can bias how the artist judges colour. Offer this for the canvas surround at minimum. |
| **High contrast** | Black or white with a saturated violet accent. | Accessibility option. Ships in Phase 1 with the rest of the UI work (P15). |

**Theme rules:**

- **The canvas surround is neutral by default.** Even in Night, the pasteboard around the artboard should be switchable to neutral grey, since coloured surrounds distort colour judgement.
- **Purple marks interaction, not content.** Selection outlines, focus rings, active tools and primary buttons are purple. Artwork, onion skins and bones use their own colours, so purple never gets confused with what the artist is drawing.
- **Keep Moho's layout, not its look.** The product vision says Fanimate follows Moho Pro's layout and workflow but draws its own icons and styling. A distinctly purple, rounded theme helps with that and with the legal position noted in the vision.
- **Corner radius and density:** 6 px panel corners, 4 px controls, with a *Compact* density option for laptop screens. Animation tools are dense, so compact must be a first-class setting.
- **Motion:** UI transitions at 120 to 160 ms, ease-out. An animation app should feel responsive. Respect the system "reduce motion" setting.

**Suggested typography** (all open licences, so they can ship in the app and repo):

| Use | Font | Why |
|---|---|---|
| UI | **Inter** | Clear at small sizes, wide language coverage, good on high-DPI screens |
| Numbers and code (timeline frame numbers, scripting console) | **JetBrains Mono** | Tabular digits so frame counts do not jitter |
| Wordmark | A geometric rounded sans such as **Nunito** or **Quicksand**, customised | Matches the friendly personality; customise the letters so the logo is not simply a font |

Check the licence on every font before bundling, and keep the licence texts in the repository.

### 1.4 Name and wordmark

- **Wordmark:** lowercase *fanimate* in a rounded geometric sans. Lowercase reads friendly and open-source; the tall *f* and *t* give shapes to play with.
- **Possible detail:** the crossbar of the *f* or the dot of the *i* is the one place to add character, such as a small bone joint or a keyframe diamond. Do only one; more becomes noisy.
- **Lockups:** icon only, icon plus wordmark horizontal, and wordmark only. Provide each in violet on light, white on violet, and white on dark.
- **Clear space:** the height of the *i* dot on every side.
- **Minimum size:** icon 16 px (for taskbars and file-type icons), wordmark 80 px wide.

**Name check before committing.** The vision already requires a legal review before 1.0. Do a trademark and domain search for "Fanimate" in Month 1, not Month 12, because every asset below is built on the name. Check the app-store, GitHub, crates.io and PyPI namespaces too.

### 1.5 App icon concepts

The icon must work at 16 px in a taskbar and at 1024 px on a splash screen, so it needs one bold shape, a limit of two or three colours, and no fine detail. Every concept below is purple and is drawn from scratch, with no resemblance to Moho's, Adobe's or Toon Boom's icons.

| # | Concept | Description | Strength | Weakness |
|---|---|---|---|---|
| **A** | **Bone-F** (recommended) | A rounded-square violet tile. A white letter *F* built from a thick stroke with a round joint node where the arm meets the stem, like a bone with a pivot. | Says *rig* and *Fanimate* at once. The joint circle survives at 16 px. | Needs care so it does not look like a generic "F" app icon. |
| **B** | **Onion-skin F** | A solid white *F* in front of two fainter, offset copies in lighter purples, suggesting onion skinning. | Says *frame-by-frame animation*. Visually distinctive. | The faint copies disappear at small sizes, so a 16 px version must drop to one copy. |
| **C** | **Bezier-F** | The *F* drawn as a single vector curve with two visible anchor points and handles. | Says *vector drawing* and appeals to illustrators. | Handles are fine detail; too technical for beginners. |
| **D** | **Keyframe diamond** | A white diamond (the universal keyframe marker) on violet, with a small trailing arc suggesting motion. | Extremely simple; scales to any size. | Generic. Many timeline tools use diamonds. |
| **E** | **Flipbook corner** | A violet page with the corner peeling up, an *F* on the visible page. | Friendly and nostalgic; speaks to education. | Detailed, and the peel does not read at 16 px. |

**Recommendation: Concept A (Bone-F), with Concept B as the splash-screen and marketing treatment.**

- A is the one app icon, in all sizes.
- B appears as a larger illustration on the splash screen and website hero, where the onion-skin layers have room to show.
- D's diamond is reused as a small brand motif: loading indicators, bullets in docs, the dot of the *i*.

**Construction brief for Concept A** (to hand to a designer):

- 1024 × 1024 canvas, rounded-square tile with a 22% corner radius, filled with a violet-500 to violet-700 diagonal gradient.
- *F* centred, made of a 12%-width stroke with round caps; the arm joins the stem at a circular node 1.5× the stroke width.
- A white F on violet-500 tile gives 4.7:1 contrast. Keep the tile at violet-600 or darker for the 16 and 32 px versions to widen that margin.
- Deliver a simplified 16 px and 32 px variant (thicker stroke, no gradient) rather than only scaling the large icon down.

### 1.6 Interface icon set

The app needs its own toolbar and panel icons (the vision requires this to avoid copying Moho's artwork).

- **Style:** 24 px grid, 1.75 px stroke, rounded caps and joins, one purple-tinted colour that follows the theme. Filled variant for the active state.
- **Core set for Phase 1 (about 120 icons):**
  - Tool groups: draw, fill, bone, layer, camera, workspace, plus every tool inside them
  - Timeline: play, step, loop, onion skin, keyframe types, X-sheet toggle
  - Layers: vector, raster, image, bone, group, switch, audio
  - File: new, open, save, import, export, recover
  - Status: saved, saving, warning, error, rig health
- **Source:** design as SVG; generate the app's icon font or sprite from the SVGs. Start from an open-licensed set (for example Lucide or Phosphor, both permissive) as a placeholder and replace tool-specific icons, such as bone, Smart Bone and onion skin, with custom ones as the artists produce them.
- **Tool-specific icons must be original.** Do not trace Moho's. The bone and point tools are the ones where a close copy would be most obvious.

### 1.7 Brand asset checklist

Everything below is needed during Phase 1. The first column shows when it is needed.

| When | Asset | Format | Notes |
|---|---|---|---|
| Month 1 | Name and trademark check | Report | Blocks everything else |
| Month 1 | Logo (icon, wordmark, lockups) | SVG | Light and dark versions, one-colour version |
| Month 1 | Colour tokens and theme file | JSON or TOML | Single source of truth, consumed by the UI and website |
| Month 1 | Repository social card and README banner | PNG 1280×640 | For GitHub; needed when the repo goes public |
| Month 2 | Favicon and app-icon set | ICO, ICNS, PNG | 16, 32, 48, 64, 128, 256, 512, 1024 px |
| Month 2 | File-type icons (`.fnm`) | ICO, ICNS, SVG | Icon on a document shape so the file format is recognisable in Explorer and Finder |
| Month 2 | Brand guidelines | Markdown in repo | Clear space, colours, do and don't, how contributors may use the logo |
| Month 3 | Splash screen and about dialog | PNG at 1x and 2x | Onion-skin illustration (Concept B) |
| Month 3 | Website and docs theme | CSS tokens | Reuse the Day and Night tokens |
| Month 4 | Interface icon set v1 | SVG | About 120 icons, placeholders allowed |
| Month 6 | Installer art | PNG, BMP | Windows installer banner, macOS DMG background, Flatpak metadata icons |
| Month 9 | Tutorial and "Coming from Moho / Animate" artwork | SVG | Mascot optional |
| Month 11 | Release assets | PNG, MP4 | Release banner and the show-reel for 1.0 |

**Optional: a mascot.** A small character, rigged in Fanimate itself, would double as a demo asset and a test rig. This is a good dogfooding task but not a Phase 1 blocker.

---

## 2. Phase 1 scope

### 2.1 In scope

Taken from the vision's roadmap, with the pain points each item addresses:

| Area | Deliverable | Pain points |
|---|---|---|
| Platform | One Rust codebase, wgpu renderer, native Windows, macOS and Linux, plus a browser build via WebAssembly | P1, P12 |
| Core engine | Scene graph, bones and deformation, animation curves, undo history, headless-capable | P12, P14 |
| File format | Open `.fnm` format with validator, self-repair and journaled autosave | P13, P14 |
| Drawing | Pressure-true vector brushes, stabiliser, clean-up tools, boolean shape tools, drawing on any frame | P9 |
| Frame-by-frame | X-sheet and timeline views, onion skin, light table, rough, clean and colour sub-layers | P9 |
| Rigging | Bones, IK and FK, Smart Bones, mesh deformation, templates, retargeting, rig health check | P7 |
| Lip sync | Bundled Rhubarb, editable lip-sync track | P6 |
| Import | SVG (full), PSD, Moho (`.moho`), Animate (`.fla`, `.xfl`) | P19 |
| Export | Video (MP4, WebM, GIF, image sequences) | P19 |
| Interface | Moho-style dockable UI, saved workspaces, HiDPI, command palette, Moho keymaps | P15 |
| Licensing and governance | MIT / Apache 2.0, public repo, roadmap and issue tracker | P21, P22 |
| Quality | Performance budget, crash-free beta, stability release train | P14, P16 |

### 2.2 Explicitly out of scope for Phase 1

Keeping these out is what makes the Phase 1 timeline believable.

| Deferred feature | Moved to |
|---|---|
| Node graph editor | Phase 2 |
| Text animators and caption tools | Phase 2 |
| Automatic colour fill (colour marks) | Phase 2 |
| Raster brush engine on par with Krita | Phase 2 (Phase 1 drawing is vector-first) |
| Scripting API (Python, TypeScript, WASM plugins) | Phase 2 |
| Local AI tools | Phase 2 |
| Lottie, sprite-sheet and game-engine export; ProRes, EXR | Phase 2 |
| Marketplace, Fanimate Hub, cloud render, cloud AI credits | Phase 3 |
| Tablet apps, live co-editing, Harmony import, education programme | Phase 4 |

**Decision flag: the vision places video export in Phase 1 but some video formats, such as ProRes, H.265 and AV1, add licence and codec complexity.** Suggested Phase 1 set: MP4 (H.264), WebM, GIF, PNG sequence. Add the rest in Phase 2. This keeps patent-encumbered codecs and their licensing questions out of the 1.0 critical path.

**Decision flag: the vision says the drawing engine paints vector or raster strokes on any layer.** Phase 1 should ship the *vector* engine and the *architecture* for raster, with raster brushes finishing in Phase 2 as the roadmap shows. This avoids building two brush engines before 1.0.

---

## 3. Workstreams

Eight workstreams run in parallel. Each lists its main tasks, its dependencies and what "done" means.

### WS1. Core engine

| Task | Detail |
|---|---|
| Scene graph | Layers, groups, transforms, parent-child, switch layers |
| Animation curves | Keyframes, interpolation types (linear, step, smooth, bezier), channels |
| Bones and deformation | Bone hierarchy, FK, IK solver, weight-based mesh deformation |
| Undo and history | Per-object undo that survives a crash; journal of every action |
| Change tracking | Dirty-flag system so only changed bones, shapes and effects are recalculated (this underpins the 60 fps budget) |
| Headless mode | Engine builds without UI code so a command-line renderer can use it |

**Done when:** a scene can be built, animated, saved and re-opened from the command line, with no UI, and the results are deterministic.

### WS2. Renderer

| Task | Detail |
|---|---|
| wgpu setup | Vulkan, Metal, DirectX 12, WebGPU backends |
| Vector rendering | GPU tessellation of paths, fills, gradients, strokes with variable width |
| Compositing | Layer blending, masks, opacity, groups |
| Shared render path | The canvas and the final export use the same renderer, so output matches the screen |
| Large-scene performance | Culling, caching of static layers, instancing |

**Done when:** the benchmark scene (see section 5) holds 60 fps on the reference mid-range laptop, on all three desktop platforms.

### WS3. File format and data safety (`.fnm`)

| Task | Detail |
|---|---|
| Specification | Published spec for the folder and zipped forms; version number from day one |
| Serialisation | Human-readable text per layer or rig, binary assets alongside |
| Validator | Broken references, duplicate and out-of-range keyframes, orphaned data |
| Self-repair | Fix what is safe automatically, report what is not |
| Autosave journal | Every 60 seconds plus every action, recover to the last action after a crash |
| Migration | Format versioning so old files always open |

**Done when:** a fuzz and corruption test suite cannot produce an unopenable file, and a forced crash loses no more than the last action.

### WS4. Drawing and frame-by-frame

| Task | Detail |
|---|---|
| Vector brush | Centreline plus width profile; pressure, tilt and speed input; natural tapered ends |
| Stabiliser | Adjustable smoothing with prediction |
| Shape and point tools | Pen, add and transform points, booleans, primitives |
| Clean-up | Auto-close gaps, merge strokes, pencil-to-ink |
| Frame-by-frame layers | Drawing on any frame, exposure, hold and repeat |
| X-sheet and timeline | Two views of the same data |
| Onion skin and light table | Colour-coded range, key-drawings-only filter, flipping |
| Sub-layers | Rough, line, colour and overlay inside one drawing |
| Pen tablet support | Wintab and Windows Ink, macOS tablet events, Linux libinput |

**Done when:** a short hand-drawn walk cycle can be created entirely in Fanimate, and a test artist reports no "locked to frame 0" or end-of-stroke curve artefacts.

### WS5. Rigging and lip sync

| Task | Detail |
|---|---|
| Bones | Creation, editing, constraints, FK and IK with per-limb blend |
| Smart Bones | Driver bones, multiple inputs, response curves, on-canvas controls |
| Mesh deformation | Auto weights, weight painting |
| Templates | Biped, quadruped, bird, face starter rigs |
| Retargeting | Move animation between rigs of different proportions |
| Rig health check | Unused bones, conflicting drivers, hidden-channel keys |
| Physics | Bone springs and gravity, bake to keyframes (stretch goal if time is short) |
| Rhubarb integration | Bundle the tool, map mouth shapes to a switch layer or Smart Bone, editable lip-sync track with waveform |

**Done when:** the benchmark rig (40 bones, 10 Smart Bones) plays at 60 fps, and a new tester rigs a character from a PSD in under 30 minutes (the success metric from the vision).

### WS6. Import and export

| Task | Detail |
|---|---|
| SVG importer | Full SVG 1.1 and SVG 2 drawing content; structure, names and styles preserved; import report |
| PSD importer | Layers, groups, blend modes, clipping, names |
| Moho importer | Layers, vector shapes, bones, Smart Bone actions, switch layers, keyframes, audio, camera |
| Animate importer | Symbols, timelines, drawings, classic and shape tweens, audio |
| Video export | MP4 (H.264), WebM, GIF, PNG sequence, with audio |
| Import test corpus | Collect a library of real-world test files (see section 5) |

**Done when:** at least 90% of the Moho test corpus opens (a Gate 1 criterion), and every import writes a report listing anything that did not map.

**Legal rule:** write importers from public documentation and the file contents only, never from the original programs' code or decompiled binaries. Log every source consulted.

### WS7. Interface

| Task | Detail |
|---|---|
| UI shell | GPU-drawn docking system, tabbed and floating panels, saved workspaces |
| Default layout | Tools left, canvas centre, Layers, Style and Actions right, timeline bottom |
| Themes | Token-based theming (Night, Day, Studio Grey, High contrast) from section 1 |
| HiDPI | Correct at 100% to 300% scaling, per-monitor |
| Command palette | Search every command and setting |
| Keymaps | Moho default, remappable; Animate and Harmony keymaps can follow in Phase 2 |
| Accessibility | Keyboard navigation, screen-reader labels where the platform allows, high-contrast theme, scalable fonts |
| First-run experience | Short interactive tutorial, "Coming from Moho / Animate" guide |

**Done when:** a Moho user, given no instructions, finds the bone tool, makes a bone and keys a pose in under five minutes in a usability session.

### WS8. Platform, release and community

| Task | Detail |
|---|---|
| Build and CI | Automated builds and tests on Windows, macOS, Linux and WASM for every change |
| Installers | Windows installer, macOS DMG (signed and notarised), Linux AppImage, Flatpak and distro packages |
| Web build | WebGPU build with documented limits; project sync with desktop |
| Telemetry (opt-in) | Crash reports and anonymous performance data, off by default, to measure the crash-free metric |
| Public repo | Licence files, contributing guide, code of conduct, issue templates |
| Public roadmap | Roadmap and voting in the issue tracker |
| Release trains | Monthly stability releases and four-monthly feature releases from the first public build |
| Docs | User manual skeleton, `.fnm` specification, architecture overview |

**Done when:** a stranger can download, install, open the sample project, and file a bug, on any of the three desktop platforms, without help from the team.

---

## 4. Milestones

The order is built around risk: prove the engine and renderer first, because everything else rests on them, and leave the long-tail compatibility work (importers) for when the core is stable. Durations are estimates and should be re-planned after Milestone 1.

| Milestone | Months | Theme | Main deliverables | Exit check |
|---|---|---|---|---|
| **M0: Ground work** | 0 to 1 | Identity and setup | Name and trademark check; brand v1 (section 1); licence and repo; governance basics; CI skeleton; legal review scoped | Repo public with README, licence, brand assets |
| **M1: Engine spike** | 1 to 3 | Prove the architecture | Core engine; wgpu renderer on three desktop platforms; `.fnm` v0 read and write; bare UI shell; benchmark scenes built | Benchmark vector scene renders at 60 fps on all three platforms; headless render works |
| **M2: Draw** | 3 to 5 | Drawing and timeline basics | Vector brush and stabiliser; shape tools; layers; timeline and keyframes; undo; autosave journal; first themes | An artist can draw, animate by keyframes, save and recover after a forced crash |
| **M3: Rig** | 5 to 7 | The reason people use Moho | Bones, FK and IK; Smart Bones; mesh deformation; Actions panel; rig health check; PSD import | A character from a PSD is rigged and animated; 40-bone rig at 60 fps |
| **M4: Frame-by-frame and voice** | 7 to 9 | Traditional animation and dialogue | X-sheet; onion skin and light table; sub-layers; Rhubarb lip sync; SVG import; video export v1 | A hand-drawn cycle and a lip-synced line are exported to MP4 |
| **M5: Bring your files** | 9 to 11 | Migration | Moho importer; Animate importer; web build; installers for all platforms; first public alpha; workspaces and command palette polish | Import corpus pass rate at or above 75%; alpha installs on a clean machine |
| **M6: Harden** | 11 to 12 | Quality gate | Bug-fix freeze; public beta; performance tuning; crash-free measurement; legal review; documentation; 1.0 release candidate | Every Gate 1 criterion in section 5 passes |

**Rules for the schedule:**

- **No milestone closes with known data-loss bugs.** Data safety (P13, P14) is a standing requirement, not an M6 task.
- **A public alpha at M5, not at M6.** The beta needs real users to produce the crash-free measurement.
- **Moho and Animate importers wait until M5 on purpose.** They depend on a stable scene model. Starting them at M1 would mean rewriting them whenever the model changes. Start collecting the test corpus in M1, though, since that costs nothing.
- **If a milestone slips, cut scope, not quality.** The first things to cut are physics (WS5) and the Animate `.fla` importer's shape tweens, in that order.

---

## 5. Gate 1 acceptance tests

Gate 1 is the exit from Phase 1. It is a release decision, not a date.

### 5.1 Criteria from the vision

| Criterion | How it is measured | Target |
|---|---|---|
| Performance budget | Benchmark scene on the reference mid-range laptop | **60 fps** with a 40-bone character and 10 Smart Bones |
| Project open time | A 5-minute episode scene, cold start | **Under 3 seconds** |
| UI responsiveness | Automated UI action timing across the main workflows | **No single action over 100 ms** |
| Stability | Opt-in crash telemetry across the beta | **99.5% crash-free sessions** |
| Moho compatibility | The Moho test corpus (below) | **90% of files open** |

### 5.2 Supporting tests added for Phase 1

| Test | Pass condition |
|---|---|
| Crash recovery | A forced kill at a random moment recovers to the last action in 100 of 100 trials |
| File corruption | Fuzzing the `.fnm` format never produces a crash; the validator repairs or reports every case |
| Cross-platform parity | The same scene renders pixel-identical, within a small tolerance, on Windows, macOS and Linux |
| Canvas versus export | Exported frames match the on-screen canvas for the benchmark scene |
| SVG conformance | A defined set of SVG test files imports with matching geometry, plus an import report for unsupported features |
| Fresh install | Installers run on clean Windows 10/11, macOS 13+ and Ubuntu, Fedora and Flatpak, and open a sample project |
| Accessibility | High-contrast theme and keyboard navigation work through the main workflows; contrast tests in section 1.2 pass |
| Usability | A first-time Moho user and a first-time non-Moho user each complete the "rig a character" task; median under 30 minutes |
| Licence audit | Every dependency and bundled font, library and model has a compatible licence recorded |
| Legal review | Interface, naming and importers reviewed (required before 1.0 in the vision) |

### 5.3 The test corpus

The 90% Moho compatibility target needs real files, so start collecting in M1.

- **Moho:** 100 or more files, from simple to complex, including known tricky cases such as the negative-keyframe bug (P13), nested switch layers and heavy Smart Bone rigs. Source them from willing community members and from permissively licensed samples.
- **Animate:** 50 or more `.fla` and `.xfl` files with symbols and tweens.
- **SVG:** the public SVG test suites plus real exports from Illustrator, Inkscape, Affinity and Figma.
- **PSD:** layered character sheets with groups, blend modes and clipping masks.
- **Consent and licensing:** record the permission for every contributed file. Never put private studio files in the public repository.

Define "opens" precisely before testing: the file loads without error, the layers, bones and timing are present, and a short render matches a reference render within an agreed tolerance. Without this definition the 90% figure is easy to hit and means little.

---

## 6. Phase 1 risks

These are in addition to the project-wide risks in the vision.

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Rust and wgpu GPU differences cause rendering bugs on some drivers | High | Medium | Test matrix with real hardware early (M1); software-rendering fallback |
| Vector tessellation on the GPU is slow or looks wrong for thick or overlapping strokes | Medium | High | Prototype stroke rendering in M1; keep the width-profile model independent of the renderer so techniques can be swapped |
| Moho and Animate importers eat unbounded time on edge cases | High | High | Define "opens" tightly; ship an import report; accept the 90% target rather than 100%; cut tweens first |
| Smart Bone behaviour does not match Moho users' expectations | Medium | High | Test against the Moho corpus rigs as soon as M3 begins; involve Moho animators in design reviews |
| Scope pressure from the whole feature list | High | High | Section 2.2 out-of-scope list is binding; any addition needs a removal |
| WebGPU limits make the browser build too weak | Medium | Low | Position as learning and review, as the vision says; ship with documented limits |
| Early contributors and the community are too few | Medium | Medium | Public repo from M0; "good first issue" labels; publish architecture docs early |
| Code quality problems, especially if AI-assisted coding is used | Medium | High | Human review on every change; benchmarks and tests in CI from M1 |
| Name or trade-dress challenge | Low | High | Trademark search in Month 1; original icons and styling; legal review in M6 |
| Codec and font licence issues | Medium | Medium | Restrict Phase 1 export to the codecs listed in section 2.2; audit fonts and libraries |

---

## 7. First 30 days

A concrete starting list, in priority order.

1. **Trademark and domain check** for "Fanimate"; reserve the domain, GitHub organisation, crates.io and PyPI names.
2. **Choose the brand direction:** confirm purple, pick one icon concept from section 1.5 and one tagline.
3. **Commission or draw the logo and Concept A icon**; produce the 16 to 1024 px set.
4. **Write the design-token file** (colours, radii, spacing, type) from section 1.2 and 1.3. This is the first file in the UI codebase.
5. **Create the public repository:** dual MIT / Apache 2.0 licence files, README with the banner, contributing guide, code of conduct, issue templates.
6. **Set up CI** that builds a "hello window" on Windows, macOS, Linux and WASM.
7. **Pick the reference hardware** for the performance budget and write down its exact specification, so "mid-range laptop" is a defined machine.
8. **Build the two benchmark scenes** (40-bone character, 5-minute episode) as the first test assets, even if only as drawings of the plan.
9. **Start the import test corpus** and the consent record for contributors.
10. **Recruit three to five advisers**: two Moho animators, one Animate user, one studio technical director. They seed the steering group and the usability tests.

---

*This breakdown should be revisited at the end of M1. By then the first real benchmark numbers will show whether the 12-month estimate and the milestone order still hold.*
