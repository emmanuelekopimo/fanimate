//! Running Rhubarb Lip Sync on a sound and keying the selected switch
//! layer from its result.
//!
//! Rhubarb is a separate program shipped next to Fanimate (or found on
//! PATH). It runs in a background thread with no console window, so the
//! editor stays responsive while it listens; the result is applied as one
//! undo step when it arrives.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

use fanimate_core::{Frame, LayerId, LayerKind};
use fanimate_editor::{MouthCue, StatusKind, parse_rhubarb};

use crate::app::Shell;

/// A lip sync in progress.
pub struct Job {
    switch: LayerId,
    start: Frame,
    sound: String,
    result: mpsc::Receiver<Result<Vec<MouthCue>, String>>,
}

/// Start lip syncing the selected switch layer to the project's sound:
/// the selected audio layer, or else the first one.
pub fn start(shell: &mut Shell) {
    if shell.lipsync.is_some() {
        shell
            .editor
            .set_status(StatusKind::Info, "Lip sync is already running");
        return;
    }
    let Some(switch) = shell.editor.switch_layer() else {
        shell
            .editor
            .set_status(StatusKind::Error, "Select a switch layer of mouth shapes first");
        return;
    };
    let Some((name, media, start)) = sound(shell) else {
        shell.editor.set_status(
            StatusKind::Error,
            "Import the dialogue as a WAV sound first (File > Import)",
        );
        return;
    };
    let Some(bytes) = shell.editor.doc.media.get(&media).map(|f| f.bytes.clone()) else {
        shell.editor.set_status(
            StatusKind::Error,
            format!("The sound file of \"{name}\" is missing"),
        );
        return;
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run(&bytes));
    });
    shell
        .editor
        .set_status(StatusKind::Info, format!("Lip sync: listening to \"{name}\"…"));
    shell.lipsync = Some(Job {
        switch,
        start,
        sound: name,
        result: rx,
    });
}

/// Each frame: apply the result once Rhubarb is done.
pub fn poll(ctx: &eframe::egui::Context, shell: &mut Shell) {
    let Some(job) = &shell.lipsync else {
        return;
    };
    let outcome = match job.result.try_recv() {
        Ok(r) => r,
        Err(mpsc::TryRecvError::Empty) => {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }
        Err(mpsc::TryRecvError::Disconnected) => Err("Rhubarb stopped unexpectedly".into()),
    };
    let Some(job) = shell.lipsync.take() else {
        return;
    };
    match outcome.and_then(|cues| shell.editor.apply_lip_sync(job.switch, &cues, job.start)) {
        Ok(0) => shell
            .editor
            .set_status(StatusKind::Success, format!("Lip synced to \"{}\"", job.sound)),
        Ok(n) => shell.editor.set_status(
            StatusKind::Warning,
            format!(
                "Lip synced to \"{}\"; {n} mouth shape(s) had no matching layer and were skipped",
                job.sound
            ),
        ),
        Err(e) => shell
            .editor
            .set_status(StatusKind::Error, format!("Lip sync failed: {e}")),
    }
}

/// The sound to listen to: its layer name, media name and start frame.
fn sound(shell: &Shell) -> Option<(String, String, Frame)> {
    let pick = |l: &fanimate_core::Layer| match &l.kind {
        LayerKind::Audio(a) => Some((l.name.clone(), a.media.clone(), a.start)),
        _ => None,
    };
    let selected = shell
        .editor
        .selection
        .iter()
        .filter_map(|id| shell.editor.doc.find(*id))
        .find_map(pick);
    selected.or_else(|| {
        let mut first = None;
        shell.editor.doc.walk(|l| {
            if first.is_none() {
                first = pick(l);
            }
        });
        first
    })
}

/// Run Rhubarb on a WAV file's bytes. Blocks; call from a thread.
fn run(wav: &[u8]) -> Result<Vec<MouthCue>, String> {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "fanimate-lipsync-{}-{}.wav",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, wav).map_err(|e| format!("could not write a temporary file: {e}"))?;
    let mut cmd = Command::new(rhubarb());
    cmd.args(["-q", "-f", "json", "--extendedShapes", "GHX"])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let output = cmd.output();
    let _ = std::fs::remove_file(&path);
    let output = output.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            "Rhubarb Lip Sync was not found; it ships with Fanimate, so reinstalling should bring it back"
                .to_owned()
        }
        _ => format!("Rhubarb could not start: {e}"),
    })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let line = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("no reason given");
        return Err(format!("Rhubarb could not read the sound ({})", line.trim()));
    }
    parse_rhubarb(&String::from_utf8_lossy(&output.stdout))
}

/// Rhubarb's program: next to Fanimate's (or in a `rhubarb` folder there,
/// as its release unpacks), or else whatever PATH finds.
fn rhubarb() -> PathBuf {
    let exe = if cfg!(windows) { "rhubarb.exe" } else { "rhubarb" };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .into_iter()
        .flat_map(|dir| [dir.join(exe), dir.join("rhubarb").join(exe)])
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(exe))
}

/// Keep a console program's window from flashing up on Windows.
#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_: &mut Command) {}
