//! CPU tessellation of draw items into a triangle mesh.

use fanimate_core::{Affine2, Color, Document, DrawItem, PathCmd, Vec2};
use lyon::math::point;
use lyon::path::AttributeIndex;
use lyon::path::Path as LyonPath;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

/// The path attribute holding the width profile.
const WIDTH: AttributeIndex = 0;

/// `[x, y, r, g, b, a, u, v]`: document-space position, linear
/// premultiplied colour, and texture coordinate (0, 0 for flat colour). A
/// plain array keeps the vertex `Pod` without unsafe code.
pub type Vertex = [f32; 8];

/// Triangle mesh ready for upload.
pub type Mesh = VertexBuffers<Vertex, u32>;

/// From index `first` on, triangles are drawn with this picture (`None`:
/// flat colour), until the next run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub first: u32,
    pub media: Option<String>,
}

pub struct Tessellator {
    fill: FillTessellator,
    stroke: StrokeTessellator,
    path: LyonPath,
    /// Where each picture's triangles start in the mesh, in order.
    pub runs: Vec<Run>,
    /// Maximum distance between a curve and its flattened version, in
    /// document units. Lower is smoother and slower.
    pub tolerance: f32,
}

impl Default for Tessellator {
    fn default() -> Self {
        Self {
            fill: FillTessellator::new(),
            stroke: StrokeTessellator::new(),
            path: LyonPath::new(),
            runs: Vec::new(),
            tolerance: 0.25,
        }
    }
}

impl Tessellator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty `mesh` and the runs, to start a frame.
    pub fn clear(&mut self, mesh: &mut Mesh) {
        mesh.vertices.clear();
        mesh.indices.clear();
        self.runs.clear();
    }

    /// Start drawing with `media` (`None`: flat colour) from here on.
    fn switch(&mut self, mesh: &Mesh, media: Option<&str>) {
        if self
            .runs
            .last()
            .map_or(media.is_none(), |r| r.media.as_deref() == media)
        {
            return;
        }
        let first = mesh.indices.len() as u32;
        match self.runs.last_mut() {
            // a run that never drew anything is replaced
            Some(last) if last.first == first => last.media = media.map(str::to_owned),
            _ => self.runs.push(Run {
                first,
                media: media.map(str::to_owned),
            }),
        }
    }

    /// The document's canvas rectangle in its background colour.
    pub fn canvas(&mut self, doc: &Document, mesh: &mut Mesh) {
        self.switch(mesh, None);
        let c = doc.background.to_linear_premultiplied();
        let base = mesh.vertices.len() as u32;
        for p in [
            Vec2::ZERO,
            Vec2::new(doc.width, 0.0),
            doc.size(),
            Vec2::new(0.0, doc.height),
        ] {
            mesh.vertices.push([p.x, p.y, c[0], c[1], c[2], c[3], 0.0, 0.0]);
        }
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Append the fills and strokes of `items` to `mesh`, back to front.
    /// Shapes that fail to tessellate are skipped and logged.
    pub fn draw_items(&mut self, items: &[DrawItem<'_>], mesh: &mut Mesh) {
        for item in items {
            if let Some(image) = &item.image {
                self.switch(mesh, Some(image.media));
                let c = color(Color::WHITE, item.opacity);
                let base = mesh.vertices.len() as u32;
                for (p, uv) in image.vertices.iter() {
                    let w = item.transform.transform_point2(*p);
                    mesh.vertices.push([w.x, w.y, c[0], c[1], c[2], c[3], uv.x, uv.y]);
                }
                let [cols, rows] = image.cells;
                let at = |c: u32, r: u32| base + r * (cols + 1) + c;
                for r in 0..rows {
                    for c in 0..cols {
                        let (a, b, d, e) = (at(c, r), at(c + 1, r), at(c, r + 1), at(c + 1, r + 1));
                        mesh.indices.extend([a, b, e, a, e, d]);
                    }
                }
                continue;
            }
            self.switch(mesh, None);
            let style = item.shape.style;
            if style.fill.is_none() && style.stroke.is_none() {
                continue;
            }
            // a width profile that does not match the path is ignored (the
            // validator reports it), so bad data draws as a plain stroke
            let widths = item
                .shape
                .widths
                .as_deref()
                .filter(|w| w.len() == item.shape.path.on_curve_count());
            self.build_path(&item.shape.path.cmds, item.transform, widths);

            if let Some(fill) = style.fill {
                let c = color(fill, item.opacity);
                let opts = FillOptions::tolerance(self.tolerance).with_fill_rule(FillRule::NonZero);
                let result = self.fill.tessellate_path(
                    &self.path,
                    &opts,
                    &mut BuffersBuilder::new(mesh, |v: FillVertex| {
                        let p = v.position();
                        [p.x, p.y, c[0], c[1], c[2], c[3], 0.0, 0.0]
                    }),
                );
                if let Err(e) = result {
                    log::warn!("fill of \"{}\" failed: {e:?}", item.shape.name);
                }
            }
            if let Some(stroke) = style.stroke {
                let c = color(stroke.color, item.opacity);
                // approximate the transform's scale for the line width
                let scale = item.transform.matrix2.determinant().abs().sqrt();
                let mut opts = StrokeOptions::tolerance(self.tolerance).with_line_width(stroke.width * scale);
                if widths.is_some() {
                    // brush strokes: width from the profile, rounded like ink
                    opts = opts
                        .with_variable_line_width(WIDTH)
                        .with_line_cap(LineCap::Round)
                        .with_line_join(LineJoin::Round);
                }
                let result = self.stroke.tessellate_path(
                    &self.path,
                    &opts,
                    &mut BuffersBuilder::new(mesh, |v: StrokeVertex| {
                        let p = v.position();
                        [p.x, p.y, c[0], c[1], c[2], c[3], 0.0, 0.0]
                    }),
                );
                if let Err(e) = result {
                    log::warn!("stroke of \"{}\" failed: {e:?}", item.shape.name);
                }
            }
        }
    }

    /// Convert our commands to a lyon path in document space, with the
    /// width profile (1 everywhere without one) as attribute [`WIDTH`].
    fn build_path(&mut self, cmds: &[PathCmd], t: Affine2, widths: Option<&[f32]>) {
        let p = |v: Vec2| {
            let w = t.transform_point2(v);
            point(w.x, w.y)
        };
        let mut b = LyonPath::builder_with_attributes(1);
        let mut open = false;
        // index of the next on-curve point, which every command but Close has
        let mut n = 0;
        let mut width = || {
            let w = widths.and_then(|w| w.get(n)).copied().unwrap_or(1.0);
            n += 1;
            [if w.is_finite() { w.max(0.0) } else { 1.0 }]
        };
        for cmd in cmds {
            match *cmd {
                PathCmd::MoveTo(v) => {
                    if open {
                        b.end(false);
                    }
                    b.begin(p(v), &width());
                    open = true;
                }
                PathCmd::LineTo(v) => {
                    let w = width();
                    if open {
                        b.line_to(p(v), &w);
                    }
                }
                PathCmd::CubicTo(c1, c2, v) => {
                    let w = width();
                    if open {
                        b.cubic_bezier_to(p(c1), p(c2), p(v), &w);
                    }
                }
                PathCmd::Close if open => {
                    b.end(true);
                    open = false;
                }
                // drawing before any MoveTo: ignore, as SVG renderers do
                _ => {}
            }
        }
        if open {
            b.end(false);
        }
        self.path = b.build();
    }
}

fn color(c: Color, opacity: f32) -> [f32; 4] {
    c.with_alpha(c.a * opacity).to_linear_premultiplied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::{evaluate, samples};

    #[test]
    fn benchmark_scene_tessellates() {
        let doc = samples::benchmark(40);
        let items = evaluate(&doc, 30.0);
        let mut mesh = Mesh::new();
        let mut t = Tessellator::new();
        t.canvas(&doc, &mut mesh);
        t.draw_items(&items, &mut mesh);
        assert!(mesh.indices.len() > 1000);
        assert_eq!(mesh.indices.len() % 3, 0);
        let n = mesh.vertices.len() as u32;
        assert!(mesh.indices.iter().all(|&i| i < n));
    }

    /// Half the mesh's height among vertices with `x` in `range`.
    fn half_height(mesh: &Mesh, range: std::ops::RangeInclusive<f32>) -> f32 {
        mesh.vertices
            .iter()
            .filter(|v| range.contains(&v[0]))
            .map(|v| v[1].abs())
            .fold(0.0, f32::max)
    }

    fn stroke_mesh(widths: Option<Vec<f32>>) -> Mesh {
        use fanimate_core::{Path, Shape, Style};
        let path = Path::new()
            .move_to(Vec2::new(0.0, 0.0))
            .line_to(Vec2::new(50.0, 0.0))
            .line_to(Vec2::new(100.0, 0.0));
        let mut shape = Shape::new("s", path, Style::default().with_stroke(Color::BLACK, 10.0));
        shape.widths = widths;
        let item = DrawItem {
            layer: fanimate_core::LayerId(1),
            index: 0,
            shape: std::borrow::Cow::Borrowed(&shape),
            part: None,
            image: None,
            transform: Affine2::IDENTITY,
            opacity: 1.0,
        };
        let mut mesh = Mesh::new();
        Tessellator::new().draw_items(&[item], &mut mesh);
        mesh
    }

    #[test]
    fn width_profile_varies_the_stroke_width() {
        let mesh = stroke_mesh(Some(vec![0.1, 1.0, 0.2]));
        assert!((half_height(&mesh, 49.0..=51.0) - 5.0).abs() < 0.3);
        assert!(half_height(&mesh, -2.0..=1.0) < 1.0);
        assert!(half_height(&mesh, 99.0..=102.0) < 1.5);
    }

    #[test]
    fn mismatched_width_profile_draws_a_plain_stroke() {
        let mesh = stroke_mesh(Some(vec![0.1]));
        assert!((half_height(&mesh, -1.0..=1.0) - 5.0).abs() < 0.3);
    }

    #[test]
    fn opacity_premultiplies_colour() {
        let c = color(Color::WHITE, 0.5);
        assert_eq!(c, [0.5, 0.5, 0.5, 0.5]);
    }
}
