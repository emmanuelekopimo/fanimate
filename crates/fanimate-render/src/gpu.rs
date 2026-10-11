//! The wgpu pipeline that draws tessellated meshes.

use fanimate_core::{Color, Vec2};

use crate::tess::{Mesh, Vertex};

const SHADER: &str = r#"
struct Globals {
    scale: vec2<f32>,
    offset: vec2<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs(@location(0) p: vec2<f32>, @location(1) c: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(p * g.scale + g.offset, 0.0, 1.0);
    o.color = c;
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    return i.color;
}
"#;

/// Maps document coordinates to clip space: `clip = doc * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub scale: Vec2,
    pub offset: Vec2,
    /// Target pixels per document unit; use for tessellation tolerance.
    pub zoom: f32,
}

impl View {
    /// Fit a document of `doc_size` centred in a `target` of pixels, with
    /// `margin` pixels of pasteboard around it.
    pub fn fit(doc_size: Vec2, target: (u32, u32), margin: f32) -> Self {
        let t = Vec2::new(target.0.max(1) as f32, target.1.max(1) as f32);
        let avail = (t - Vec2::splat(2.0 * margin)).max(Vec2::ONE);
        let zoom = (avail / doc_size.max(Vec2::ONE)).min_element();
        Self::centered(doc_size, t, zoom)
    }

    /// Document centred in the target at an exact zoom.
    pub fn centered(doc_size: Vec2, target: Vec2, zoom: f32) -> Self {
        Self::looking_at(doc_size / 2.0, target, zoom)
    }

    /// The document point `center` in the middle of the target, at `zoom`
    /// target pixels per document unit. This is how the canvas pans.
    pub fn looking_at(center: Vec2, target: Vec2, zoom: f32) -> Self {
        // y points down in documents and up in clip space
        let scale = Vec2::new(2.0 * zoom / target.x.max(1.0), -2.0 * zoom / target.y.max(1.0));
        let offset = -center * scale;
        Self { scale, offset, zoom }
    }
}

/// A texture to draw into, with its size in pixels.
#[derive(Clone, Copy)]
pub struct RenderTarget<'a> {
    pub view: &'a wgpu::TextureView,
    pub size: (u32, u32),
}

pub struct Renderer {
    format: wgpu::TextureFormat,
    sample_count: u32,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    msaa: Option<(wgpu::TextureView, (u32, u32))>,
}

impl Renderer {
    /// `format` is the target texture format; `sample_count` is 1 or 4.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, sample_count: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fanimate shapes"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fanimate globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fanimate globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fanimate globals"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fanimate shapes"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fanimate shapes"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: sample_count,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            format,
            sample_count,
            pipeline,
            globals,
            bind_group,
            vertices: empty_buffer(device, wgpu::BufferUsages::VERTEX),
            indices: empty_buffer(device, wgpu::BufferUsages::INDEX),
            msaa: None,
        }
    }

    /// Draw `mesh` into `target`, clearing it to `clear`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: RenderTarget<'_>,
        view: &View,
        mesh: &Mesh,
        clear: Color,
    ) {
        let RenderTarget { view: target, size } = target;
        let g = [view.scale.x, view.scale.y, view.offset.x, view.offset.y];
        queue.write_buffer(&self.globals, 0, bytemuck::cast_slice(&g));
        upload(
            device,
            queue,
            &mut self.vertices,
            wgpu::BufferUsages::VERTEX,
            bytemuck::cast_slice(&mesh.vertices),
        );
        upload(
            device,
            queue,
            &mut self.indices,
            wgpu::BufferUsages::INDEX,
            bytemuck::cast_slice(&mesh.indices),
        );

        let (color_view, resolve) = if self.sample_count > 1 {
            if self.msaa.as_ref().is_none_or(|(_, s)| *s != size) {
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("fanimate msaa"),
                    size: wgpu::Extent3d {
                        width: size.0.max(1),
                        height: size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: self.sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                self.msaa = Some((tex.create_view(&Default::default()), size));
            }
            (&self.msaa.as_ref().expect("created above").0, Some(target))
        } else {
            (target, None)
        };

        let c = clear.to_linear_premultiplied();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fanimate frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fanimate shapes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c[0] as f64,
                            g: c[1] as f64,
                            b: c[2] as f64,
                            a: c[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !mesh.indices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}

fn empty_buffer(device: &wgpu::Device, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fanimate mesh"),
        size: 4096,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Write `data` into `buf`, growing it (to the next power of two) if needed.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &mut wgpu::Buffer,
    usage: wgpu::BufferUsages,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }
    if (data.len() as u64) > buf.size() {
        let size = (data.len() as u64).next_power_of_two();
        *buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fanimate mesh"),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }
    // write_buffer needs a multiple of 4 bytes; f32 and u32 data always is
    queue.write_buffer(buf, 0, data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_centres_and_flips_y() {
        let v = View::fit(Vec2::new(100.0, 50.0), (200, 100), 0.0);
        assert_eq!(v.zoom, 2.0);
        let clip = |p: Vec2| p * v.scale + v.offset;
        assert!(clip(Vec2::ZERO).abs_diff_eq(Vec2::new(-1.0, 1.0), 1e-6));
        assert!(clip(Vec2::new(100.0, 50.0)).abs_diff_eq(Vec2::new(1.0, -1.0), 1e-6));
    }

    #[test]
    fn fit_letterboxes_tall_targets() {
        let v = View::fit(Vec2::new(100.0, 100.0), (400, 200), 0.0);
        assert_eq!(v.zoom, 2.0);
        let clip = |p: Vec2| p * v.scale + v.offset;
        // 200 px wide square in a 400 px target spans clip x -0.5..0.5
        assert!(clip(Vec2::ZERO).abs_diff_eq(Vec2::new(-0.5, 1.0), 1e-6));
    }
}
