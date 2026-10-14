# Importing Moho and Animate projects

Fanimate opens Moho and Adobe Animate projects as new Fanimate projects: **File › Open Moho or Animate Project…** (Ctrl+Shift+O), a file given on the command line, or `fanimate-cli import`. The result is an ordinary unsaved project; save it to keep it. Nothing is ever written back to the original file.

After every import the status bar lists what did not come through exactly ("2 masks left out; 1 shape tween held still"), so you know what to check. The same list prints from `fanimate-cli import`.

The importers are written from inspecting files we may use (see the clean-room rule in [CONTRIBUTING.md](../CONTRIBUTING.md)) and live in `crates/fanimate-import`.

## Moho (`.moho`, `.mohoproj`)

Moho 12 and later save a project as JSON (`.mohoproj`), usually inside a ZIP archive (`.moho`). Anime Studio 11's `.anime` and `.animeproj` are the same format. Older text projects (`.anme`) are not supported: open them in Moho and save again.

| Moho | In Fanimate |
|---|---|
| Canvas size, frame rate, frame range, background | The same. Moho's frame 1 is Fanimate's frame 0; Moho's frame 0 (the rest pose) is each channel's base value |
| Vector layers | Vector layers. Curves keep their shape: each point's smoothness becomes curve handles |
| Fills and outlines | Fill colour, outline colour and width, from the shape or its library style |
| Point animation | Point keys on the shapes |
| Group layers | Groups |
| Bone layers and bones | Bone layers with the same bones, parents, lengths, angle limits and strength; bones are reordered so parents come first |
| Bone angle animation | Bone keys, with the rest pose as the base value |
| Layer binding, point binding | Rigid binding to the bone, or per-point weights |
| Flexible binding (no binding) | Smooth binding with automatic weights |
| Switch layers | Switch layers with their keys |
| Image layers | Image layers, when the picture is beside the project or in an `images` folder there |
| Layer position, rotation, scale, flip, origin, opacity, visibility | The same; the origin becomes the pivot |
| Interpolation | Smooth, linear and step. Ease, Bézier and similar become smooth; noisy becomes linear |

Left out, and listed after the import: actions and Smart Bones (their poses are not in the project's main data), bones that move or stretch (they keep their rest place and length), bones that change parent, bone targets, masks, blend modes, cycle keys (held), animated visibility, text, sound, particle settings and 3D layers.

## Adobe Animate (`.fla`, `.xfl`)

Since Flash CS5 a project is a folder of XML files (XFL); a `.fla` is that folder in a ZIP archive. To import an `.xfl`, pick the `.xfl` file inside the project folder. A `.fla` saved by Flash CS4 or earlier is a different format: open it in Animate and save it again.

| Animate | In Fanimate |
|---|---|
| Stage size, frame rate, background, timeline length | The same |
| Layers and layer folders | Layers and groups, in the same order |
| Drawings (shapes, drawing objects, groups) | Vector shapes: fills rebuilt from Animate's edges, then lines on top |
| Rectangle and oval objects | Shapes |
| A layer with one drawing | A vector layer |
| A layer whose drawings change by keyframe | A frame-by-frame layer, one drawing per keyframe |
| A layer with one symbol on every keyframe | A group holding the symbol's layers, its keyframes as transform keys |
| Classic tweens | Linear (or smooth with easing) motion between the keys, including extra turns |
| Graphic symbols | Their layers, playing from their first frame |
| Symbols inside drawings | Flattened into the drawing at their first frame |
| Bitmaps | Image layers, from the library's original PNG or JPEG |
| Alpha colour effect | Opacity |

Left out, and listed after the import: text (convert to shapes first), gradients and bitmap fills (shown as solid colours), dashed and other styled lines (drawn solid), masks (the masked layers show unmasked), tint and brightness effects, filters, blend modes, skew, shape tweens and newer motion tweens (held at their keyframes), sounds, scripts, scenes after the first, and pictures stored only in Animate's own format.

## Measuring against the test corpus

`fanimate-cli corpus <folder>` imports every Moho and Animate file under a folder and prints, for each, whether it imported cleanly, imported with losses (and which), or failed. It is the first step of the [corpus definition of "opens"](../corpus/README.md#what-opens-means); comparing renders with the original program is a separate step.

```sh
cargo run --release -p fanimate-cli -- corpus corpus/moho
cargo run --release -p fanimate-cli -- render walk.moho -o walk.png --frame 12   # renders an import directly
```

Results so far (development samples, not the consented corpus, which is still being collected):

| Files | Imported | With losses listed | Failed |
|---|---|---|---|
| 1 Moho rig with walk cycle (MIT-licensed sample) | 1 | 1 (actions, bone movement, a mask) | 0 |
| 205 Animate files (Mozilla Shumway's Apache-licensed test files, an MPL-licensed game's assets) | 205 | 178 (mostly scripts and text in test files) | 0 |
