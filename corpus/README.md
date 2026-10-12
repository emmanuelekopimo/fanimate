# Import test corpus

Real files from Moho, Adobe Animate, SVG editors and Photoshop, used to test Fanimate's importers. The Gate 1 target is that **90% of the Moho corpus opens** ([Phase 1 plan, section 5.3](../docs/roadmap/phase-1.md#53-the-test-corpus)). The importers arrive later (PSD in M3, SVG in M4, Moho and Animate in M5); collecting starts now because it costs nothing and takes time.

## What goes here

```
corpus/
  CONSENT.md    one row per file: where it came from and the permission to use it
  moho/         .moho and .anime files
  animate/      .fla and .xfl files
  svg/          .svg files
  psd/          .psd files
```

Each file sits in its format's folder under a short, descriptive name, for example `moho/switch-layers-nested.moho`. Folders are created with their first file.

## Rules

- **Every file needs a row in [CONSENT.md](CONSENT.md) before it is committed**, recording who made it, who gave permission, the licence or terms, and the date. No row, no file.
- **Only files that may be redistributed publicly go in this repository**: the contributor's own work under a licence that allows it (CC0, CC BY, MIT or similar), or published samples whose licence allows redistribution. Never studio files, client work, or anything shared privately.
- Files we may test with but not redistribute are kept outside the repository by the maintainer who received them, and listed in CONSENT.md with `private` in the Location column. Results from them can be reported; the files cannot be shared.
- Never add files made with pirated or leaked software, and never files extracted from a program's installation (its bundled samples belong to its vendor).
- Keep each file under 20 MB. Larger files are listed in CONSENT.md and stored privately.

## Wanted

From the [Phase 1 plan](../docs/roadmap/phase-1.md#53-the-test-corpus):

- **Moho:** 100 or more files, simple to complex, including the negative-keyframe bug (P13), nested switch layers and heavy Smart Bone rigs, from as many Moho versions as possible.
- **Animate:** 50 or more `.fla` and `.xfl` files with symbols, classic tweens and shape tweens.
- **SVG:** the public SVG test suites, plus real exports from Illustrator, Inkscape, Affinity and Figma.
- **PSD:** layered character sheets with groups, blend modes and clipping masks.

To contribute a file, open an issue first with what it is, which program and version made it, and the licence you offer it under.

## What "opens" means

Proposed definition, to be agreed before the first importer is measured. A corpus file **opens** when all of these hold:

1. It imports with no error and no crash, and the result passes the `.fnm` validator with no unresolved errors.
2. Every layer, bone and keyframe in the source is present, or is listed in the import report as not mapped.
3. A render of three agreed frames (first, middle, last) matches a reference render made in the original program within the agreed pixel tolerance.
4. The scene's frame rate, length and canvas size match the source.

A file that imports but fails (2) to (4) counts as **partial**, not open, so the 90% figure means what it says.
