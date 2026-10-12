# The `.fnm` project format

**Current version: 1. Status: draft.** The format may still change before Fanimate 1.0, but every change that older readers would misread bumps the version, and Fanimate always opens files written by earlier versions. See [Version history](#version-history).

The reference implementation is the [`fanimate-fnm`](../../crates/fanimate-fnm/) crate.

## Design goals

- **Open and documented.** Anyone can read and write `.fnm` without Fanimate's code.
- **Version-control friendly.** Plain JSON, split into one file per top-level layer, so Git diffs are readable and two artists rarely edit the same file.
- **Never lose work.** Saves are atomic; loads are validated and damage is repaired or reported, never silently kept.
- **Lossless.** Saving and loading a project gives back exactly the same project.

## Layout

A project is a folder whose name ends in `.fnm`:

```text
scene.fnm/
├── manifest.json
├── document.json
└── layers/
    ├── 0002.json
    └── 0005.json
```

A single-file form (the same folder in a zip archive) is planned and will get its own extension.

All files are UTF-8 JSON. Whitespace is not significant; Fanimate writes them indented, with a final newline.

## `manifest.json`

Identifies the folder as a Fanimate project.

```json
{
  "format": "fnm",
  "version": 1,
  "generator": "fanimate 0.0.6"
}
```

| Field | Type | Meaning |
|---|---|---|
| `format` | string | Always `"fnm"`. A folder without it is not a project. |
| `version` | integer | Format version. Readers must refuse versions newer than they support. |
| `generator` | string | Program that wrote the file. Informational only. |

## `document.json`

Canvas, timing, and the order of top-level layers.

```json
{
  "name": "Hello Fanimate",
  "width": 1280.0,
  "height": 720.0,
  "fps": 24.0,
  "start": 0,
  "end": 47,
  "background": "#1C0B3D",
  "next_layer_id": 3,
  "layers": ["layers/0002.json"]
}
```

| Field | Type | Meaning |
|---|---|---|
| `name` | string | Project name |
| `width`, `height` | number | Canvas size in document units (pixels at 100% zoom). Positive. |
| `fps` | number | Frames per second. Positive. |
| `start`, `end` | integer | Inclusive frame range that plays and renders. `0 <= start <= end`. |
| `background` | [colour](#colours) | Canvas colour |
| `next_layer_id` | integer | Next unused layer id. Greater than every id in the project. |
| `layers` | array of strings | Top-level layer files relative to the project folder, **bottom to top** (the last is drawn on top). |

## Layer files

Each file in `layers/` holds one top-level layer. Child layers of groups and bone layers are stored inline inside their parent, not in separate files. Fanimate names the file after the layer id (`layers/0002.json` for id 2), so renaming a layer does not rename its file.

### Common layer fields

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Unique within the project; stable for the layer's life |
| `name` | string | Display name |
| `visible` | boolean | Optional, default `true` |
| `opacity` | [channel](#channels) of number | 0 to 1. Multiplies with ancestors' opacity. |
| `transform` | object | `position` (channel of [vector](#vectors)), `rotation` (channel of number, radians), `scale` (channel of vector) |
| `type` | string | `"vector"`, `"group"` or `"bone"`; decides the remaining fields |

A layer's transform applies scale, then rotation, then position, relative to its parent.

### `"type": "vector"`

| Field | Type | Meaning |
|---|---|---|
| `shapes` | array of [shapes](#shapes) | Drawn first to last |

### `"type": "group"`

| Field | Type | Meaning |
|---|---|---|
| `children` | array of layers | Drawn first to last |

### `"type": "bone"`

A skeleton. Shapes on vector layers anywhere inside `children` can bind to its bones.

| Field | Type | Meaning |
|---|---|---|
| `skeleton.bones` | array of [bones](#bones) | Parents always come before their children |
| `children` | array of layers | Drawn first to last |

## Shapes

```json
{
  "name": "middle arm",
  "path": [{"M": [542.0, 322.0]}, {"L": [684.0, 322.0]}, "Z"],
  "style": {"fill": "#FFFFFF", "stroke": {"color": "#5EEAD4", "width": 6.0}},
  "bone": 0
}
```

| Field | Type | Meaning |
|---|---|---|
| `name` | string | Display name |
| `path` | array of [path commands](#paths) | Outline, in the layer's space |
| `style.fill` | colour | Optional. Non-zero fill rule. |
| `style.stroke` | object | Optional. `color` and `width` (document units). |
| `bone` | integer | Optional. Index of the bone this shape follows, in the nearest enclosing bone layer. |
| `widths` | array of numbers | Optional (version 1). The stroke's [width profile](#width-profiles). |

A bound shape's points are stored in the **rest pose**. At each frame the shape moves rigidly with its bone: from the bone's rest transform to its current transform. Weighted deformation (one point following several bones) will be added in a later version.

### Width profiles

Brush strokes vary in width along their length, from pen pressure and tapered ends. A shape records this as `widths`: one number per **on-curve point** of its path, each a multiple of `style.stroke.width`.

```json
{
  "name": "Stroke",
  "path": [{"M": [10.0, 40.0]}, {"C": [[20.0, 30.0], [40.0, 30.0], [50.0, 40.0]]}, {"L": [90.0, 40.0]}],
  "style": {"stroke": {"color": "#1C0B3D", "width": 8.0}},
  "widths": [0.05, 1.0, 0.3]
}
```

- On-curve points are the points of `M`, `L` and the end point of `C`, in order; `Z` has none. The example has three, so `widths` has three values, and the stroke is 0.4, 8 and 2.4 units wide at them.
- Between two points the width changes smoothly along the segment.
- Values are finite and not negative. A profile whose length does not match the path is ignored: the stroke is drawn at `style.stroke.width` everywhere.
- A stroke with a profile has round caps and round joins. Without one, caps are flat (butt) and joins are mitred.
- `widths` has no effect on fills, or on a shape without a stroke.

## Paths

A path is a list of commands, named after SVG's path letters. Points are `[x, y]`.

| Command | JSON | Meaning |
|---|---|---|
| Move | `{"M": [x, y]}` | Start a new subpath |
| Line | `{"L": [x, y]}` | Straight line to the point |
| Cubic | `{"C": [[x1, y1], [x2, y2], [x, y]]}` | Cubic Bézier: two control points, then the end point |
| Close | `"Z"` | Close the subpath |

Every subpath starts with `M`. Drawing commands with no open subpath (before the first `M`, or after a `Z` and before the next `M`) are ignored.

## Bones

```json
{
  "name": "arm",
  "parent": 0,
  "origin": [570.0, 350.0],
  "length": 110.0,
  "angle": {"base": 0.0, "keys": [{"frame": 12, "value": -0.35}]}
}
```

| Field | Type | Meaning |
|---|---|---|
| `name` | string | Display name |
| `parent` | integer | Optional. Index of the parent bone; must be lower than this bone's index. |
| `origin` | vector | Joint position in the parent bone's space (x along the parent bone), or in the bone layer's space for a root bone |
| `length` | number | Bone length |
| `angle` | channel of number | Rotation relative to the parent, radians. `base` is the **rest pose**. |

Each bone's local space has its origin at the joint and +x pointing along the bone. Its transform at a frame is `parent_world × translate(origin) × rotate(angle)`.

## Channels

Any animatable value is a channel:

```json
{"base": 0.0, "keys": [{"frame": 0, "value": 0.0}, {"frame": 12, "value": 1.0, "interp": "linear"}]}
```

| Field | Type | Meaning |
|---|---|---|
| `base` | value | The value when there are no keys. For bone angles, also the rest pose. |
| `keys` | array | Optional, default empty. Sorted by `frame`, at most one key per frame, no negative frames. |
| `keys[].frame` | integer | Frame number |
| `keys[].value` | value | Value at that frame |
| `keys[].interp` | string | How the value moves **from this key to the next**: `"smooth"` (default, ease in and out), `"linear"` or `"step"` (hold). Optional. |

Sampling: before the first key, the first key's value; after the last key, the last key's value; between keys, interpolated as set by the earlier key. Frames can be sampled fractionally.

## Values

### Vectors

`[x, y]` as two numbers.

### Colours

sRGB, straight (not premultiplied) alpha. Written as a hex string, `"#RRGGBB"` or `"#RRGGBBAA"`, when 8 bits per channel hold the colour exactly. Otherwise written as `[r, g, b, a]` with each channel from 0 to 1, so saving never changes a colour. Readers must accept both, and `"#RGB"`.

### Coordinates and units

- Document space has **y pointing down**, origin at the top-left corner of the canvas.
- Angles are in **radians**; positive turns clockwise on screen.
- Lengths are in document units.
- All numbers must be finite.

## Saving

To make saving safe against crashes and full disks, a writer should:

1. Write the complete project to a temporary sibling folder (Fanimate uses `.scene.fnm.saving`).
2. Rename the existing project out of the way (`.scene.fnm.old`).
3. Rename the temporary folder into place.
4. Delete the old copy.

If step 3 fails, put the old copy back. At no point is the only copy of the project half-written.

## Crash recovery copies

While a project is open, Fanimate keeps a copy of it in its data folder, updated after every action, so a crash loses at most the action being written. The copy is an ordinary `.fnm` project folder (`project.fnm`) next to a `session.json` saying where the project is saved and whether it had unsaved changes. Each file in the copy is replaced atomically. The copy is deleted when Fanimate closes normally. Nothing else in this specification depends on it.

## Validation

Readers should validate on load. Fanimate's validator reports, and on request repairs, the following:

| Problem | Severity | Repair |
|---|---|---|
| Keyframe on a negative frame | Error | Remove the key |
| Keys out of order | Error | Sort by frame |
| Two keys on the same frame | Error | Keep the later one in the file |
| Non-finite value or key | Error | Reset the value; remove the key |
| Duplicate layer id | Error | Renumber the later layer |
| `next_layer_id` not above every id | Error | Set to highest id + 1 |
| Bone parent not before the bone | Error | Make the bone a root |
| Non-positive `fps`, `width` or `height`; bad frame range | Error | Reset to defaults |
| Width profile whose length does not match the path's on-curve points | Error | Remove the profile |
| Negative or non-finite value in a width profile | Error | Non-finite becomes 1, negative becomes 0 |
| Shape bound to a bone that does not exist, or outside any bone layer | Warning | Unbind the shape |
| Keyframe after the `end` frame | Warning | None; the key is kept |
| Opacity outside 0 to 1 | Warning | None; clamped when drawn |

## Versioning

- A change that older readers would misread bumps `version`.
- Readers refuse files with a newer version than they support, rather than guessing.
- Fanimate opens every older version, converting on load.
- Format changes are proposed in an issue and documented here before they ship (see [GOVERNANCE.md](../../GOVERNANCE.md)).

## Version history

| Version | First written by | Changes | Converting older files |
|---|---|---|---|
| 0 | Fanimate 0.0.1 | First version: layers, shapes, bones, channels | |
| 1 | Fanimate 0.0.6 | Shapes may have a stroke width profile (`widths`), written by the brush | Version 0 files need no conversion. A version 0 reader would ignore `widths` and draw brush strokes at one width, so the version was bumped. |
