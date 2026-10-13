//! Exporting animation: rendering a project to a video file, an animated
//! GIF or a PNG sequence, and the render queue that does it in a separate
//! process.
//!
//! Frames are rendered headless with `fanimate-render`, the same renderer
//! as the canvas. Video is encoded by FFmpeg, run as a separate program
//! (`ffmpeg`, shipped next to Fanimate) and fed raw frames on its standard
//! input; nothing of FFmpeg is linked into Fanimate. GIF and PNG need no
//! other program.
//!
//! The queue lives in a folder: one subfolder per job holding a copy of the
//! project as it was when queued, its settings and its progress, plus the
//! order the jobs run in. The editor adds jobs; the queue window (`fanimate
//! --render-queue`) runs them, so a render goes on after the editor closes.

pub mod ffmpeg;
pub mod job;
pub mod queue;
pub mod run;
pub mod sound;

pub use job::{Format, Settings};
pub use queue::{Entry, Job, Queue, State, Status};
pub use run::{ExportError, export};
