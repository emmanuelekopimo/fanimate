# Fanimate user manual

This is the outline of the manual, with the essentials of each chapter. Chapters grow as testers ask questions; open an issue for anything that is missing or unclear.

## 1. Getting started

- Install: see [Installing and updating](../install.md).
- First start: choose a keyboard shortcut set (Fanimate's standard shortcuts, or Moho's) and a theme. Both can be changed in Edit › Preferences.
- The screen follows Moho Pro's layout: tools on the left, the canvas in the middle, the timeline along the bottom, Properties, Style and Actions on the right above Layers. Every panel can be moved, tabbed, closed and reopened from the Window menu. **Window › Workspace** arranges the panels for animating, drawing, rigging, frame-by-frame work or review.
- **F1** or **Ctrl+Shift+P** opens the command palette: type part of any command's name.
- Right-click anything (layers, keys, the canvas) for what you can do with it.

## 2. Projects

- **File › New**, **Open**, **Save** and **Save As**. A project is a `.fnm` folder of readable files ([format](../spec/fnm.md)).
- Fanimate keeps a copy of your work after every change. If it closes unexpectedly, the next start offers to recover it.
- **File › Open Moho or Animate Project…** brings in `.moho` and `.fla` files: see [Importing](../import.md).
- **File › Import** (Ctrl+R) adds pictures (PNG, JPEG), Photoshop files (as layers), SVG drawings and WAV sound to the open project.

## 3. Drawing

- Freehand brush with pressure and a stabiliser, rectangle, ellipse, pen and Add Point tools, eraser, paint bucket and eyedropper.
- Select Points and Transform Points edit drawings point by point. Combine Shapes joins, subtracts and intersects.
- The Style panel sets colours, line width and taper for new drawings.

## 4. Animating

- Every value on the timeline can have keys. Editing an animated value sets a key on the current frame; editing an unanimated one changes it for the whole animation.
- Drag keys to retime them; right-click a key for its interpolation (smooth, linear, step).
- Points of a drawing can be keyed too, to change its shape over time.

## 5. Rigging

- Bone layers hold a skeleton. **Add Bone** draws chains; **Select Bone** edits the rest pose; **Manipulate Bones** poses with inverse kinematics (Alt for one bone) and always keys the frame.
- Bind artwork to a bone (Ctrl+B), smoothly to several (Ctrl+Shift+B), or paint weights (W). Pictures bend with bones on a mesh.
- **Actions** and **Smart Bones** store poses that bones drive: a turning head, a correcting elbow.
- The rig health check in Properties lists bones that move nothing and Smart Bones that need attention.

## 6. Frame-by-frame and lip sync

- Frame-by-frame layers hold one drawing per exposure, with rough, colour, line and overlay parts. **New Drawing** (F7) copies the drawing on show.
- Onion skin (Alt+O) and the light table (Alt+L) help drawing between frames. The X-sheet panel shows exposures down the page.
- Switch layers show one child at a time: mouths, hands, eyes. With a switch layer selected, **Lip Sync** (in the Layer menu or the layer's right-click menu) listens to a WAV dialogue track and keys its mouths.

## 7. Exporting

- **File › Export Video** (Ctrl+Shift+R): MP4 or MKV (H.264 with sound), animated GIF or a PNG sequence, at any size and frame range.
- Renders run in the render queue, a window of its own that keeps working after Fanimate closes.

## 8. Keyboard shortcuts

**Help › Keyboard Shortcuts** lists every shortcut of the set in use. Edit › Keyboard Shortcuts switches between Fanimate's standard set and Moho's.

## 9. Fanimate in the browser

The [web build](../web.md) opens, plays and checks projects without installing anything.
