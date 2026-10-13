//! Headless Fanimate tool. Runs the same engine and renderer as the app
//! with no window, for render servers, CI and batch work.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use fanimate_core::eval::Evaluator;
use fanimate_core::{Document, samples};
use fanimate_render::{Gpu, Mesh, Offscreen, Tessellator, render_to_rgba};

const USAGE: &str = "\
usage:
  fanimate-cli sample <hello|benchmark> <out.fnm> [--bones N]
  fanimate-cli validate <project.fnm> [--repair]
  fanimate-cli render <project.fnm> -o <out.png> [--frame N] [--width W] [--height H]
  fanimate-cli bench [--bones N] [--frames N] [--gpu [--width W] [--height H]]";

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("sample") => sample(&args[1..]),
        Some("validate") => validate(&args[1..]),
        Some("render") => render(&args[1..]),
        Some("bench") => bench(&args[1..]),
        Some("-V" | "--version") => {
            println!("fanimate-cli {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("-h" | "--help") | None => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command {other}\n{USAGE}")),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fanimate-cli: {e}");
            ExitCode::from(2)
        }
    }
}

type CmdResult = Result<ExitCode, String>;

/// Split arguments into positionals and `--flag value` pairs.
struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(args: &[String], takes_value: &[&str]) -> Result<Self, String> {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a.starts_with('-') {
                let value = if takes_value.contains(&a.as_str()) {
                    Some(it.next().ok_or(format!("{a} needs a value"))?.clone())
                } else {
                    None
                };
                flags.push((a.clone(), value));
            } else {
                positional.push(a.clone());
            }
        }
        Ok(Self { positional, flags })
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(f, _)| f == name)
    }

    fn get<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, String> {
        match self.flags.iter().find(|(f, _)| f == name) {
            Some((_, Some(v))) => v
                .parse()
                .map(Some)
                .map_err(|_| format!("bad value for {name}: {v}")),
            _ => Ok(None),
        }
    }
}

fn sample(args: &[String]) -> CmdResult {
    let a = Args::parse(args, &["--bones"])?;
    let [kind, out] = a.positional.as_slice() else {
        return Err(USAGE.into());
    };
    let doc = match kind.as_str() {
        "hello" => samples::hello(),
        "benchmark" => samples::benchmark(a.get("--bones")?.unwrap_or(40)),
        other => return Err(format!("unknown sample {other}; use hello or benchmark")),
    };
    fanimate_fnm::save(&doc, out).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(ExitCode::SUCCESS)
}

fn validate(args: &[String]) -> CmdResult {
    let a = Args::parse(args, &[])?;
    let [path] = a.positional.as_slice() else {
        return Err(USAGE.into());
    };
    let (doc, report) = if a.has("--repair") {
        let (doc, report) = fanimate_fnm::load_and_repair(path).map_err(|e| e.to_string())?;
        if report.repaired_count() > 0 {
            fanimate_fnm::save(&doc, path).map_err(|e| e.to_string())?;
        }
        (doc, report)
    } else {
        fanimate_fnm::load(path).map_err(|e| e.to_string())?
    };
    print!("{report}");
    if report.is_clean() {
        println!();
    }
    println!(
        "{}: {} layers, {} issue(s), {} repaired",
        doc.name,
        doc.layer_count(),
        report.issues.len(),
        report.repaired_count()
    );
    Ok(if report.unresolved_errors().next().is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn render(args: &[String]) -> CmdResult {
    let a = Args::parse(args, &["-o", "--frame", "--width", "--height"])?;
    let [path] = a.positional.as_slice() else {
        return Err(USAGE.into());
    };
    let out: PathBuf = a.get::<String>("-o")?.ok_or("render needs -o <out.png>")?.into();
    let (doc, report) = fanimate_fnm::load(path).map_err(|e| e.to_string())?;
    if !report.is_clean() {
        eprint!("{report}");
    }
    let (width, height) = output_size(&doc, a.get("--width")?, a.get("--height")?);
    let frame: i32 = a.get("--frame")?.unwrap_or(doc.start);

    let started = Instant::now();
    let gpu = Gpu::headless().map_err(|e| e.to_string())?;
    let pixels = render_to_rgba(&gpu, &doc, frame as f32, width, height).map_err(|e| e.to_string())?;
    image::save_buffer(&out, &pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    let info = gpu.adapter.get_info();
    println!(
        "rendered frame {frame} at {width}x{height} to {} in {:.0} ms on {} ({:?})",
        out.display(),
        started.elapsed().as_secs_f64() * 1000.0,
        info.name,
        info.backend
    );
    Ok(ExitCode::SUCCESS)
}

/// Document size by default; one dimension given scales the other to match.
fn output_size(doc: &Document, w: Option<u32>, h: Option<u32>) -> (u32, u32) {
    let aspect = doc.width / doc.height;
    match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, (w as f32 / aspect).round() as u32),
        (None, Some(h)) => ((h as f32 * aspect).round() as u32, h),
        (None, None) => (doc.width.round() as u32, doc.height.round() as u32),
    }
}

/// CPU-side benchmark: evaluate and tessellate every frame, plus file
/// save and load. `--gpu` adds the full render path without a window (see
/// [`bench_gpu`]); `fanimate --bench` measures it in the app, UI included.
fn bench(args: &[String]) -> CmdResult {
    let a = Args::parse(args, &["--bones", "--frames", "--width", "--height"])?;
    let bones: usize = a.get("--bones")?.unwrap_or(40);
    let frames: u32 = a.get("--frames")?.unwrap_or(2000);
    let doc = samples::benchmark(bones);

    let mut evaluator = Evaluator::default();
    let mut tess = Tessellator::new();
    let mut items = Vec::new();
    let mut mesh = Mesh::new();
    let (mut eval_t, mut tess_t) = (0.0, 0.0);
    let mut triangles = 0;
    for i in 0..frames {
        let frame = doc.start as f32 + (i as f32 * 0.5) % doc.frame_count() as f32;
        let t0 = Instant::now();
        evaluator.evaluate_into(&doc, frame, &mut items);
        let t1 = Instant::now();
        tess.clear(&mut mesh);
        tess.canvas(&doc, &mut mesh);
        tess.draw_items(&items, &mut mesh);
        let t2 = Instant::now();
        eval_t += (t1 - t0).as_secs_f64();
        tess_t += (t2 - t1).as_secs_f64();
        triangles = mesh.indices.len() / 3;
    }
    let per = |t: f64| t * 1000.0 / frames as f64;

    let dir = std::env::temp_dir().join(format!("fanimate-bench-{}", std::process::id()));
    let path = dir.join("bench.fnm");
    let t0 = Instant::now();
    fanimate_fnm::save(&doc, &path).map_err(|e| e.to_string())?;
    let save_t = t0.elapsed();
    let t0 = Instant::now();
    fanimate_fnm::load(&path).map_err(|e| e.to_string())?;
    let load_t = t0.elapsed();
    let _ = std::fs::remove_dir_all(&dir);

    let total = per(eval_t) + per(tess_t);
    println!(
        "{}: {} shapes, {triangles} triangles, {frames} frames",
        doc.name,
        items.len()
    );
    println!("  evaluate     {:>8.3} ms/frame", per(eval_t));
    println!("  tessellate   {:>8.3} ms/frame", per(tess_t));
    println!("  CPU total    {:>8.3} ms/frame  (60 fps budget: 16.7 ms)", total);
    println!("  save .fnm    {:>8.1} ms", save_t.as_secs_f64() * 1000.0);
    println!(
        "  load .fnm    {:>8.1} ms (with validation)",
        load_t.as_secs_f64() * 1000.0
    );
    if a.has("--gpu") {
        let size = (
            a.get("--width")?.unwrap_or(1280),
            a.get("--height")?.unwrap_or(800),
        );
        bench_gpu(&doc, frames, size)?;
    }
    if cfg!(debug_assertions) {
        println!("  note: debug build; use --release for real numbers");
    }
    Ok(ExitCode::SUCCESS)
}

/// Longest the GPU part of the benchmark runs, so a slow software renderer
/// on a CI machine does not hold up a release for minutes.
const GPU_BENCH_LIMIT: Duration = Duration::from_secs(10);

/// Evaluate, tessellate and draw frames offscreen with 4x MSAA, as the
/// canvas does, waiting for the GPU after each one. Waiting gives up the
/// overlap a window gets between CPU and GPU work, so the frame rate is a
/// conservative figure for the same machine.
fn bench_gpu(doc: &Document, frames: u32, (width, height): (u32, u32)) -> Result<(), String> {
    let gpu = Gpu::headless().map_err(|e| e.to_string())?;
    let mut target = Offscreen::new(&gpu, width, height);
    let frame_at = |i: u32| doc.start as f32 + (i as f32 * 0.5) % doc.frame_count() as f32;

    // the first frames include pipeline and buffer setup
    for i in 0..10 {
        target.draw(&gpu, doc, frame_at(i));
    }
    target.wait(&gpu).map_err(|e| e.to_string())?;

    let mut times = Vec::with_capacity(frames as usize);
    let started = Instant::now();
    for i in 0..frames {
        let t0 = Instant::now();
        target.draw(&gpu, doc, frame_at(i));
        target.wait(&gpu).map_err(|e| e.to_string())?;
        times.push(t0.elapsed());
        if started.elapsed() > GPU_BENCH_LIMIT {
            break;
        }
    }
    let total = started.elapsed().as_secs_f64();
    times.sort_unstable();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];

    let info = gpu.adapter.get_info();
    println!(
        "  GPU render   {:>8.1} fps at {width}x{height}, 4x MSAA ({} frames, {} triangles)",
        times.len() as f64 / total,
        times.len(),
        target.triangles()
    );
    println!(
        "  frame time   {:>8.3} ms median, {:.3} ms 95th percentile, {:.3} ms slowest",
        ms(times[times.len() / 2]),
        ms(p95),
        ms(times[times.len() - 1])
    );
    println!(
        "  adapter      {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );
    if gpu.is_software() {
        println!("  note: software renderer; these numbers say nothing about GPU speed");
    }
    Ok(())
}
