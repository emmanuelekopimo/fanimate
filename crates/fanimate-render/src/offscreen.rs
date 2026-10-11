//! GPU setup and headless rendering to an image.

use fanimate_core::eval::Evaluator;
use fanimate_core::{Color, Document};

use crate::{Mesh, RenderError, RenderTarget, Renderer, Tessellator, View};

/// An open GPU device.
///
/// Holds on to the instance as well: some drivers crash if the instance is
/// destroyed while a device created from it is still alive.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub instance: wgpu::Instance,
}

impl Gpu {
    /// Open a device on `instance`, able to present to `surface` if given.
    pub async fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("fanimate"),
                // WebGL2 limits keep the browser build in reach
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await?;
        Ok(Self {
            device,
            queue,
            adapter,
            instance,
        })
    }

    /// Open a device for offscreen work, with no window.
    pub fn headless() -> Result<Self, RenderError> {
        pollster::block_on(Self::new(instance(), None))
    }
}

/// Create the wgpu instance with Fanimate's backend choice.
///
/// On Windows this is DirectX 12, with OpenGL as a fallback for old GPUs.
/// Vulkan is left out there because some older Intel drivers (seen on HD
/// Graphics 520) crash the whole process while the Vulkan driver loads, which
/// cannot be caught. Elsewhere wgpu's primary backends are used (Vulkan,
/// Metal, WebGPU). The `WGPU_BACKEND` environment variable overrides this,
/// e.g. `WGPU_BACKEND=vulkan`.
pub fn instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends();
    wgpu::Instance::new(desc.with_env())
}

/// The graphics backends Fanimate uses (see [`instance`]), honouring
/// `WGPU_BACKEND`. Also used to configure the app window's renderer.
pub fn backends() -> wgpu::Backends {
    wgpu::Backends::from_env().unwrap_or(if cfg!(windows) {
        wgpu::Backends::DX12 | wgpu::Backends::GL
    } else {
        wgpu::Backends::PRIMARY
    })
}

/// Render `doc` at `frame` to straight-alpha sRGB RGBA8 pixels, with the
/// canvas filling the image. Uses 4x MSAA, like the interactive canvas.
pub fn render_to_rgba(
    gpu: &Gpu,
    doc: &Document,
    frame: f32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = Renderer::new(&gpu.device, format, 4);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fanimate offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = View::fit(doc.size(), (width, height), 0.0);

    let mut items = Vec::new();
    Evaluator::default().evaluate_into(doc, frame, &mut items);
    let mut mesh = Mesh::new();
    let mut tess = Tessellator::new();
    tess.tolerance = 0.25 / view.zoom;
    tess.canvas(doc, &mut mesh);
    tess.draw_items(&items, &mut mesh);

    let target_view = target.create_view(&Default::default());
    renderer.render(
        &gpu.device,
        &gpu.queue,
        RenderTarget {
            view: &target_view,
            size: (width, height),
        },
        &view,
        &mesh,
        Color::TRANSPARENT,
    );

    // Copy out. Rows in the buffer must be 256-byte aligned.
    let row = width * 4;
    let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fanimate readback"),
        size: padded as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);

    let (tx, rx) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|_| RenderError::Readback)?;
    rx.recv()
        .map_err(|_| RenderError::Readback)?
        .map_err(|_| RenderError::Readback)?;

    let data = buffer.get_mapped_range(..).map_err(|_| RenderError::Readback)?;
    let mut pixels = Vec::with_capacity((row * height) as usize);
    for y in 0..height as usize {
        let start = y * padded as usize;
        pixels.extend_from_slice(&data[start..start + row as usize]);
    }
    drop(data);
    buffer.unmap();
    unpremultiply(&mut pixels);
    Ok(pixels)
}

/// The GPU blends premultiplied; image files expect straight alpha.
fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3];
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::samples;

    /// Needs a GPU (or a software adapter such as lavapipe or WARP).
    /// Skips, with a note, on machines with neither.
    #[test]
    fn renders_hello_sample() {
        let gpu = match Gpu::headless() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("skipping GPU test: {e}");
                return;
            }
        };
        let doc = samples::hello();
        let (w, h) = (320, 180);
        let px = render_to_rgba(&gpu, &doc, 0.0, w, h).unwrap();
        assert_eq!(px.len(), (w * h * 4) as usize);

        let at = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [px[i], px[i + 1], px[i + 2], px[i + 3]]
        };
        // corner is the violet-950 background, #1C0B3D
        let bg = at(2, 2);
        assert!(
            bg[0].abs_diff(0x1C) <= 2 && bg[1].abs_diff(0x0B) <= 2 && bg[2].abs_diff(0x3D) <= 2,
            "{bg:?}"
        );
        assert_eq!(bg[3], 255);
        // the F's stem is white: stem centre is at x = 570 of 1280
        let stem = at(570 * w / 1280, h / 2 + 30);
        assert!(stem.iter().all(|&c| c >= 250), "{stem:?}");
    }

    #[test]
    fn unpremultiply_restores_straight_alpha() {
        let mut px = [64, 0, 128, 128];
        unpremultiply(&mut px);
        assert_eq!(px, [128, 0, 255, 128]);
    }
}
