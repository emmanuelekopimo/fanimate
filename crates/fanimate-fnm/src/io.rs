//! Reading and writing `.fnm` project folders.

use std::fs;
use std::path::{Path, PathBuf};

use fanimate_core::{Color, Document, Frame, Layer};
use serde::{Deserialize, Serialize};

use crate::validate::{self, Report};
use crate::{FnmError, Result};

pub const FORMAT_NAME: &str = "fnm";
/// Version written by this build. Readers accept this version and older.
pub const FORMAT_VERSION: u32 = 0;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    #[serde(default)]
    generator: String,
}

/// `document.json`: everything except the layers themselves.
#[derive(Serialize, Deserialize)]
struct DocumentFile {
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

fn layer_file(layer: &Layer) -> String {
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

    Ok(Document {
        name: file.name,
        width: file.width,
        height: file.height,
        fps: file.fps,
        start: file.start,
        end: file.end,
        background: file.background,
        layers,
        next_layer_id: file.next_layer_id,
    })
}

/// Save a project folder.
///
/// The project is written to a temporary folder next to `path` and swapped
/// into place only once every file is written, so a crash or full disk
/// mid-save never leaves a half-written project.
pub fn save(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
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

    write_json(
        &dir.join("manifest.json"),
        &Manifest {
            format: FORMAT_NAME.into(),
            version: FORMAT_VERSION,
            generator: concat!("fanimate ", env!("CARGO_PKG_VERSION")).into(),
        },
    )?;
    write_json(
        &dir.join("document.json"),
        &DocumentFile {
            name: doc.name.clone(),
            width: doc.width,
            height: doc.height,
            fps: doc.fps,
            start: doc.start,
            end: doc.end,
            background: doc.background,
            next_layer_id: doc.next_layer_id,
            layers: doc.layers.iter().map(layer_file).collect(),
        },
    )?;
    for layer in &doc.layers {
        write_json(&dir.join(layer_file(layer)), layer)?;
    }
    Ok(())
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

fn io_err(path: &Path, source: std::io::Error) -> FnmError {
    FnmError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
