<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/logo/fanimate-lockup-dark.svg">
    <img src="brand/logo/fanimate-lockup-light.svg" alt="Fanimate" height="80">
  </picture>
</p>

<p align="center"><strong>Free to animate.</strong><br>
Open-source 2D character animation. Rig it. Draw it. Move it.</p>

---

Fanimate is a free, open-source 2D character animation app. It keeps what animators love about Moho Pro (bone rigging, Smart Bones, vector puppets), fixes the problems that push people away from it, and adds the best tools from Toon Boom Harmony and Adobe Animate.

> **Status: planning.** There is no code yet. This repository holds the product vision, the Phase 1 plan and the brand. Development starts with Phase 1 (Foundation), which ends at version 1.0.

## Goals

- **Moho's rigging, Harmony's drawing and pipeline, Animate's ease of use**, in one app.
- **Familiar on day one:** Moho Pro's layout, tool names and shortcuts, with Fanimate's own code, icons and styling.
- **Free on every platform:** Windows, macOS and Linux natively, plus a browser build. Tablets after 1.0.
- **Fast:** one Rust codebase with GPU rendering (wgpu), built to hold 60 fps with complex rigs.
- **Never lose work:** an open, documented `.fnm` file format with self-repair and crash-proof autosave.
- **Open everything:** source, file format, roadmap and API.

## Documents

| Document | What it covers |
|---|---|
| [Product vision](docs/product-vision.md) | The problem, principles, full feature specification, AI, business model, roadmap and risks |
| [Phase 1: Foundation](docs/roadmap/phase-1.md) | Branding, scope, workstreams, milestones and Gate 1 acceptance tests for version 1.0 |
| [Brand guidelines](brand/README.md) | Logo, colours, themes, typography and how to rebuild the assets |

## Roadmap

| Phase | Duration (estimate) | Focus | Ends at |
|---|---|---|---|
| **1. Foundation** | About 12 months | Engine and renderer, drawing and frame-by-frame, rigging, lip sync, SVG, PSD, Moho and Animate import, video export | **Version 1.0** |
| 2. Production | About 6 months | Node graph, text animation, auto colour fill, raster brushes, scripting API, local AI tools, Lottie and game export | A pilot short film ships |
| 3. Studio | About 6 months | Fanimate Hub, review and approvals, cloud rendering, marketplace | Studio-ready |
| 4. Reach | Ongoing | Tablet apps, live co-editing, Harmony import, education | — |

Each phase ends at a quality gate, not a date. See the [product vision](docs/product-vision.md#roadmap-risks-and-success-metrics) for details.

## Repository layout

```
fanimate/
├── docs/
│   ├── product-vision.md     Product vision
│   └── roadmap/
│       └── phase-1.md        Phase 1 breakdown
├── brand/                    Logo, icons, design tokens, fonts and build script
├── LICENSE-APACHE
├── LICENSE-MIT
└── README.md
```

## Contributing

The project is at the planning stage. Feedback on the vision and the Phase 1 plan is welcome; open an issue to discuss. Contribution guidelines and a code of conduct will be added before development starts.

## Licence

Fanimate is dual-licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Fanimate by you, as defined in the Apache-2.0 licence, shall be dual-licensed as above, without any additional terms or conditions.

The Inter font in `brand/fonts/` is licensed separately under the [SIL Open Font License 1.1](brand/fonts/Inter-OFL.txt). The Fanimate name and logo identify this project; see the [brand guidelines](brand/README.md#licence).
