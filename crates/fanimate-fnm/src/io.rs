//! Reading and writing `.fnm` project folders.

use std::fs;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use fanimate_core::{Color, Document, Frame, Layer, LayerKind, MediaFile};
use serde::{Deserialize, Serialize};

use crate::validate::{self, Report};
use crate::{FnmError, Result};

pub const FORMAT_NAME: &str = "fnm";
/// Version written by this build. Readers accept this version and older.
///
/// - 0: the Phase 1 engine spike.
/// - 1: shapes may carry a stroke width profile (`widths`), written by the
///   brush. Version 0 files need no conversion.
/// - 2: shapes may carry point keyframes (`path_keys`). Older files need
///   no conversion.
/// - 3: layers may carry a label colour (`label`). Older files need no
///   conversion; their layers are shown with a colour picked by id.
pub const FORMAT_VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
pub(crate) struct Manifest {
    format: String,
    version: u32,
    #[serde(default)]
    generator: String,
}

/// `document.json`: everything except the layers themselves.
#[derive(Serialize, Deserialize)]
pub(crate) struct DocumentFile {
    name: String,
    width: f32,
    height: f32,
    fps: f32,
    start: Frame,
    end: Frame,
    background: Color,
    next_layer_id: u32,
    /// Top-level layer files, bottom to top, relative to the project folder.
    layers: Vec<String>,
}

pub(crate) fn layer_file(layer: &Layer) -> String {
    format!("layers/{:04}.json", layer.id.0)
}

/// Load a project and validate it. Problems are returned in the report but
/// not repaired; use [`load_and_repair`] to fix what can be fixed safely.
pub fn load(path: impl AsRef<Path>) -> Result<(Document, Report)> {
    let doc = read(path.as_ref())?;
    let report = validate::validate(&doc);
    Ok((doc, report))
}

/// Load a project, repairing anything the validator can fix safely.
pub fn load_and_repair(path: impl AsRef<Path>) -> Result<(Document, Report)> {
    let mut doc = read(path.as_ref())?;
    let report = validate::repair(&mut doc);
    Ok((doc, report))
}

fn read(dir: &Path) -> Result<Document> {
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(FnmError::NotFnm(dir.to_path_buf()));
    }
    let manifest: Manifest = read_json(&manifest_path)?;
    if manifest.format != FORMAT_NAME {
        return Err(FnmError::NotFnm(dir.to_path_buf()));
    }
    if manifest.version > FORMAT_VERSION {
        return Err(FnmError::UnsupportedVersion {
            found: manifest.version,
            supported: FORMAT_VERSION,
        });
    }

    let file: DocumentFile = read_json(&dir.join("document.json"))?;
    let layers = file
        .layers
        .iter()
        .map(|rel| read_json::<Layer>(&dir.join(rel)))
        .collect::<Result<Vec<_>>>()?;

    let mut doc = Document {
        name: file.name,
        width: file.width,
        height: file.height,
        fps: file.fps,
        start: file.start,
        end: file.end,
        background: file.background,
        layers,
        next_layer_id: file.next_layer_id,
        action_edit: None,
        media: Default::default(),
    };
    // pictures used by image layers; a missing one is reported by the
    // validator and drawn as a grey placeholder, never a failed load
    for name in media_names(&doc) {
        if let Ok(bytes) = fs::read(dir.join(&name)) {
            doc.media.insert(name, Arc::new(MediaFile { bytes }));
        }
    }
    Ok(doc)
}

/// Whether `name` is a media file name Fanimate reads and writes: a plain
/// file name inside the project's `media` folder, so a damaged project can
/// never point outside its own folder.
pub fn is_media_name(name: &str) -> bool {
    name.strip_prefix("media/").is_some_and(|f| {
        !f.is_empty()
            && f.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !f.starts_with('.')
    })
}

/// The media files image layers use, with safe names.
pub(crate) fn media_names(doc: &Document) -> Vec<String> {
    let mut names = Vec::new();
    doc.walk(|l| {
        if let LayerKind::Image(img) = &l.kind
            && is_media_name(&img.media)
            && !names.contains(&img.media)
        {
            names.push(img.media.clone());
        }
    });
    names
}

/// Write the media files `doc` uses into project folder `dir`, skipping
/// any already there (their names follow their content).
pub(crate) fn write_media(doc: &Document, dir: &Path) -> Result<()> {
    let names = media_names(doc);
    if names.is_empty() {
        return Ok(());
    }
    let media_dir = dir.join("media");
    fs::create_dir_all(&media_dir).map_err(|e| io_err(&media_dir, e))?;
    for name in names {
        let Some(file) = doc.media.get(&name) else {
            continue;
        };
        let path = dir.join(&name);
        if !path.is_file() {
            let tmp = path.with_extension("tmp");
            fs::write(&tmp, &file.bytes).map_err(|e| io_err(&tmp, e))?;
            fs::rename(&tmp, &path).map_err(|e| io_err(&path, e))?;
        }
    }
    Ok(())
}

/// Save a project folder.
///
/// The project is written to a temporary folder next to `path` and swapped
/// into place only once every file is written, so a crash or full disk
/// mid-save never leaves a half-written project.
pub fn save(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    // an action being edited is put away first, so the timeline is saved
    let doc = &*doc.mainline();
    let path = path.as_ref();
    let tmp = sibling(path, "saving");
    let old = sibling(path, "old");
    let _ = fs::remove_dir_all(&tmp);

    write_all(doc, &tmp)?;

    let had_old = path.exists();
    if had_old {
        let _ = fs::remove_dir_all(&old);
        fs::rename(path, &old).map_err(|e| io_err(path, e))?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        // put the previous version back before reporting the failure
        if had_old {
            let _ = fs::rename(&old, path);
        }
        return Err(io_err(path, e));
    }
    if had_old {
        let _ = fs::remove_dir_all(&old);
    }
    Ok(())
}

fn write_all(doc: &Document, dir: &Path) -> Result<()> {
    let layers_dir = dir.join("layers");
    fs::create_dir_all(&layers_dir).map_err(|e| io_err(&layers_dir, e))?;

    write_json(&dir.join("manifest.json"), &manifest())?;
    write_json(&dir.join("document.json"), &document_file(doc))?;
    for layer in &doc.layers {
        write_json(&dir.join(layer_file(layer)), layer)?;
    }
    write_media(doc, dir)
}

pub(crate) fn manifest() -> Manifest {
    Manifest {
        format: FORMAT_NAME.into(),
        version: FORMAT_VERSION,
        generator: concat!("fanimate ", env!("CARGO_PKG_VERSION")).into(),
    }
}

pub(crate) fn document_file(doc: &Document) -> DocumentFile {
    DocumentFile {
        name: doc.name.clone(),
        width: doc.width,
        height: doc.height,
        fps: doc.fps,
        start: doc.start,
        end: doc.end,
        background: doc.background,
        next_layer_id: doc.next_layer_id,
        layers: doc.layers.iter().map(layer_file).collect(),
    }
}

/// Write a JSON file so that it is never seen half-written: write a
/// temporary file beside it, then rename it over the old one.
pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    write_json(&tmp, value)?;
    fs::rename(&tmp, path).map_err(|e| io_err(path, e))
}

fn sibling(path: &Path, tag: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project.fnm".into());
    path.with_file_name(format!(".{name}.{tag}"))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    serde_json::from_str(&text).map_err(|source| FnmError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| FnmError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|e| io_err(path, e))
}

pub(crate) fn io_err(path: &Path, source: std::io::Error) -> FnmError {
    FnmError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::eval::evaluate;
    use fanimate_core::samples;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fanimate-fnm-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip_preserves_document() {
        let dir = temp_dir("round-trip");
        let path = dir.join("bench.fnm");
        let doc = samples::benchmark(40);
        save(&doc, &path).unwrap();
        let (loaded, report) = load(&path).unwrap();
        assert_eq!(loaded, doc);
        assert!(report.is_clean(), "{report}");
        fs::remove_dir_all(dir).unwrap();
    }

    /// Every file in a project folder, by path relative to it.
    fn project_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let rel = path.strip_prefix(root).unwrap().to_path_buf();
                    out.push((rel, fs::read(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }

    /// The engine's headless promise (WS1): a scene built and animated in
    /// code, saved and reopened with no UI, evaluates identically on every
    /// frame, and saving it again writes the same bytes.
    #[test]
    fn reopened_project_is_deterministic() {
        let dir = temp_dir("deterministic");
        let doc = samples::benchmark(40);
        let (first, second) = (dir.join("a.fnm"), dir.join("b.fnm"));
        save(&doc, &first).unwrap();
        let (loaded, _) = load(&first).unwrap();
        save(&loaded, &second).unwrap();
        assert_eq!(project_files(&first), project_files(&second));

        let mut half_frames = doc.start * 2..=doc.end * 2;
        assert!(half_frames.all(|f| {
            let frame = f as f32 / 2.0;
            let (a, b) = (evaluate(&doc, frame), evaluate(&loaded, frame));
            a.len() == b.len()
                && a.iter().zip(&b).all(|(a, b)| {
                    a.layer == b.layer
                        && a.shape == b.shape
                        && a.transform == b.transform
                        && a.opacity == b.opacity
                })
        }));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_replaces_existing_project() {
        let dir = temp_dir("replace");
        let path = dir.join("p.fnm");
        save(&samples::benchmark(40), &path).unwrap();
        let hello = samples::hello();
        save(&hello, &path).unwrap();
        assert_eq!(load(&path).unwrap().0, hello);
        // no temporary folders left behind
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_folders_that_are_not_projects() {
        let dir = temp_dir("not-fnm");
        assert!(matches!(load(&dir), Err(FnmError::NotFnm(_))));
        fs::remove_dir_all(dir).unwrap();
    }

    /// Files from every released format version must keep opening. Each
    /// fixture was written by the release that used that version.
    #[test]
    fn opens_version_0_projects() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-v0.fnm");
        let (doc, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(doc, samples::hello());
    }

    #[test]
    fn brush_strokes_round_trip() {
        let dir = temp_dir("brush");
        let path = dir.join("p.fnm");
        let mut doc = samples::hello();
        let samples: Vec<_> = (0..40)
            .map(|i| {
                fanimate_core::BrushSample::new(
                    fanimate_core::Vec2::new(i as f32 * 5.0, (i as f32).sin() * 20.0),
                    0.7,
                )
            })
            .collect();
        let opts = fanimate_core::BrushOptions {
            color: fanimate_core::Color::BLACK,
            size: 8.0,
            taper: 15.0,
            tolerance: 0.5,
        };
        let stroke = fanimate_core::brush::build_stroke(&samples, &opts).unwrap();
        assert!(stroke.widths.is_some());
        let layer = doc.new_layer("Ink", fanimate_core::LayerKind::vector(vec![stroke]));
        doc.layers.push(layer);
        save(&doc, &path).unwrap();
        let manifest = fs::read_to_string(path.join("manifest.json")).unwrap();
        assert!(manifest.contains(&format!("\"version\": {FORMAT_VERSION}")));
        let (loaded, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(loaded, doc);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn point_keys_round_trip() {
        use fanimate_core::{Interpolation, LayerKind, Vec2};
        let dir = temp_dir("point-keys");
        let path = dir.join("p.fnm");
        let mut doc = samples::hello();
        let mut shape = fanimate_core::Shape::new(
            "Blob",
            fanimate_core::Path::circle(Vec2::new(100.0, 100.0), 40.0),
            fanimate_core::Style::fill(fanimate_core::Color::BLACK),
        );
        let mut squashed = shape.path.clone();
        squashed.move_points(&[1, 3].into_iter().collect(), Vec2::new(0.0, -20.0));
        shape.key_path(0, shape.path.clone(), Interpolation::Smooth);
        shape.key_path(12, squashed, Interpolation::Linear);
        let layer = doc.new_layer("Blob", LayerKind::vector(vec![shape]));
        doc.layers.push(layer);
        save(&doc, &path).unwrap();
        let (loaded, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(loaded, doc);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn labels_round_trip() {
        let dir = temp_dir("labels");
        let path = dir.join("p.fnm");
        let mut doc = samples::hello();
        doc.layers[0].label = Some(fanimate_core::Label::Pink);
        save(&doc, &path).unwrap();
        let (loaded, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(loaded, doc);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn image_layers_and_their_pictures_round_trip() {
        let dir = temp_dir("media");
        let path = dir.join("p.fnm");
        let mut doc = samples::hello();
        let bytes = b"not really a png".to_vec();
        let name = MediaFile::name_for(&bytes, "png");
        doc.media.insert(name.clone(), Arc::new(MediaFile { bytes }));
        let layer = doc.new_layer(
            "pic",
            LayerKind::Image(fanimate_core::Image::new(name.clone(), 20.0, 10.0)),
        );
        doc.layers.push(layer);
        // a picture no layer uses is not saved
        doc.media
            .insert("media/unused.png".into(), Arc::new(MediaFile { bytes: vec![1] }));
        save(&doc, &path).unwrap();
        assert!(path.join(&name).is_file());
        assert!(!path.join("media/unused.png").exists());
        let (loaded, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(loaded.layers, doc.layers);
        assert_eq!(loaded.media.len(), 1);
        assert_eq!(loaded.media[&name], doc.media[&name]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn media_names_stay_inside_the_project() {
        assert!(is_media_name("media/0123abcd-10.png"));
        for bad in [
            "../x.png",
            "media/../x.png",
            "media/a/b.png",
            "/etc/passwd",
            "media/",
            "media/.hidden",
        ] {
            assert!(!is_media_name(bad), "{bad}");
        }
    }

    #[test]
    fn opens_version_1_projects() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/strokes-v1.fnm");
        let (doc, report) = load(&path).unwrap();
        assert!(report.is_clean(), "{report}");
        let mut shapes = 0;
        doc.walk(|l| {
            if let fanimate_core::LayerKind::Vector { shapes: s } = &l.kind {
                shapes += s.iter().filter(|s| s.widths.is_some()).count();
            }
        });
        assert!(shapes > 0);
    }

    #[test]
    fn rejects_newer_format_versions() {
        let dir = temp_dir("newer");
        let path = dir.join("p.fnm");
        save(&samples::hello(), &path).unwrap();
        fs::write(path.join("manifest.json"), r#"{"format":"fnm","version":99}"#).unwrap();
        assert!(matches!(
            load(&path),
            Err(FnmError::UnsupportedVersion { found: 99, .. })
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
