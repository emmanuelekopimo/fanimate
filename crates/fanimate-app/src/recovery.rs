//! Crash recovery in the app: keep the open document's journal up to date,
//! and offer work left behind by a crash when the app starts.
//!
//! The journal itself (where and how it is written) is in
//! `fanimate_fnm::journal`.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{self, Id, RichText};
use fanimate_editor::{Editor, StatusKind};
use fanimate_fnm::{Journal, Recoverable};

use crate::app::Shell;

/// While there are unsaved changes, record at least this often. Catches
/// edits that are not separate actions, such as typing in a field.
const BACKSTOP: Duration = Duration::from_secs(10);

/// Where journals live: beside the app's settings.
pub fn root() -> Option<PathBuf> {
    eframe::storage_dir("Fanimate").map(|dir| dir.join("recovery"))
}

/// The journal state the app keeps.
pub struct Recovery {
    root: Option<PathBuf>,
    journal: Option<Journal>,
    /// The editor revision last recorded; `None` before the first record.
    recorded: Option<u64>,
    recorded_at: Instant,
    error_shown: bool,
    /// Work from crashed sessions, offered in a dialog.
    pub offered: Vec<Recoverable>,
}

impl Recovery {
    /// With no `root` (no data folder, or the benchmark), nothing is
    /// journalled.
    pub fn new(root: Option<PathBuf>) -> Self {
        let offered = root
            .as_deref()
            .map(fanimate_fnm::find_recoverable)
            .unwrap_or_default();
        Self {
            root,
            journal: None,
            recorded: None,
            recorded_at: Instant::now(),
            error_shown: false,
            offered,
        }
    }
}

/// Start a new journal session for the editor's document, ending the
/// previous one (its document was saved or deliberately closed).
pub fn start(shell: &mut Shell) {
    if let Some(old) = shell.recovery.journal.take() {
        old.close();
    }
    shell.recovery.recorded = None;
    shell.recovery.error_shown = false;
    let Some(root) = &shell.recovery.root else {
        return;
    };
    match Journal::start(root) {
        Ok(journal) => shell.recovery.journal = Some(journal),
        Err(e) => shell
            .editor
            .set_status(StatusKind::Warning, format!("Crash recovery is off: {e}")),
    }
    record(shell);
}

/// Record the document as it is now.
pub fn record(shell: &mut Shell) {
    if let Some(journal) = &shell.recovery.journal {
        let history = shell.editor.history_record();
        let ed = &shell.editor;
        journal.record(&ed.doc, ed.path.as_deref(), ed.dirty, Some(history));
    }
    let ed = &shell.editor;
    shell.recovery.recorded = Some(ed.revision());
    shell.recovery.recorded_at = Instant::now();
}

/// Each frame: record once an action has finished (not in the middle of a
/// drag or stroke), and every few seconds while there are unsaved changes.
pub fn tick(ctx: &egui::Context, shell: &mut Shell) {
    if shell.recovery.journal.is_none() {
        return;
    }
    let busy = ctx.input(|i| i.pointer.any_down()) || shell.editor.is_drawing() || shell.editor.is_erasing();
    if !busy {
        let changed = shell.recovery.recorded != Some(shell.editor.revision());
        let due = shell.editor.dirty && shell.recovery.recorded_at.elapsed() >= BACKSTOP;
        if changed || due {
            record(shell);
        }
    }
    let error = shell.recovery.journal.as_ref().and_then(Journal::error);
    match error {
        Some(e) if !shell.recovery.error_shown => {
            shell.recovery.error_shown = true;
            let msg = format!("Crash recovery could not save a copy of your work: {e}");
            shell.editor.set_status(StatusKind::Error, msg);
        }
        Some(_) => {}
        None => shell.recovery.error_shown = false,
    }
}

/// The app is closing normally: nothing to recover next time.
pub fn end(shell: &mut Shell) {
    if let Some(journal) = shell.recovery.journal.take() {
        journal.close();
    }
}

/// The "recover unsaved work?" dialog shown at startup after a crash.
pub fn dialog(ctx: &egui::Context, shell: &mut Shell) {
    if shell.recovery.offered.is_empty() {
        return;
    }
    let muted = shell.palette.text_muted;
    let mut recover = None;
    let mut discard = None;
    let mut later = false;
    egui::Modal::new(Id::new("recover-work")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.label(RichText::new("Recover unsaved work?").size(16.0).strong());
        ui.label(
            RichText::new("Fanimate closed before this work was saved. It was kept up to the last change.")
                .color(muted),
        );
        ui.add_space(8.0);
        for (i, r) in shell.recovery.offered.iter().enumerate() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&r.info.name).strong());
                let place = r
                    .info
                    .path
                    .as_ref()
                    .map_or("Never saved".to_owned(), |p| p.display().to_string());
                ui.label(RichText::new(format!("{place}, changed {}", ago(r.info.updated))).color(muted));
                ui.horizontal(|ui| {
                    let primary = egui::Button::new(RichText::new("Recover").color(shell.palette.on_button))
                        .fill(shell.palette.button);
                    if ui.add(primary).clicked() {
                        recover = Some(i);
                    }
                    if ui.button("Discard").clicked() {
                        discard = Some(i);
                    }
                });
            });
        }
        ui.add_space(6.0);
        if ui
            .button("Decide later")
            .on_hover_text("Keep the work for next time Fanimate starts")
            .clicked()
        {
            later = true;
        }
    });

    if let Some(i) = discard {
        shell.recovery.offered.remove(i).discard();
    }
    if later {
        shell.recovery.offered.clear();
    }
    let Some(i) = recover else {
        return;
    };
    let r = shell.recovery.offered.remove(i);
    match r.load() {
        Ok((doc, _report)) => {
            let path = r.info.path.clone();
            shell.replace_editor(Editor::recovered(doc, path, r.load_history()));
            // the new session holds the work now; only then drop the old one
            if let Some(journal) = &shell.recovery.journal {
                journal.flush();
            }
            r.discard();
            // one document at a time: the rest wait for the next start
            shell.recovery.offered.clear();
        }
        Err(e) => {
            let msg = format!("Could not recover \"{}\": {e}", r.info.name);
            shell.editor.set_status(StatusKind::Error, msg);
        }
    }
}

/// "3 minutes ago" for a time in seconds since 1970.
fn ago(secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = now.saturating_sub(secs);
    let (n, unit) = match s {
        0..60 => return "just now".into(),
        60..3600 => (s / 60, "minute"),
        3600..86_400 => (s / 3600, "hour"),
        _ => (s / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_reads_naturally() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(ago(now), "just now");
        assert_eq!(ago(now - 60), "1 minute ago");
        assert_eq!(ago(now - 7200), "2 hours ago");
        assert_eq!(ago(now - 3 * 86_400), "3 days ago");
    }
}
