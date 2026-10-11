//! CPU tessellation of draw items into a triangle mesh.

use fanimate_core::{Affine2, Color, Document, DrawItem, PathCmd, Vec2};
use lyon::math::point;
use lyon::path::Path as LyonPath;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
};

/// `[x, y, r, g, b, a]`: document-space position and linear, premultiplied
/// colour. A plain array keeps the vertex `Pod` without unsafe code.
pub type Vertex = [f32; 6];

/// Triangle mesh ready for upload.
pub type Mesh = VertexBuffers<Vertex, u32>;

pub struct Tessellator {
    fill: FillTessellator,
    stroke: StrokeTessellator,
    path: LyonPath,
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
            tolerance: 0.25,
        }
    }
}

impl Tessellator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The document's canvas rectangle in its background colour.
    pub fn canvas(&mut self, doc: &Document, mesh: &mut Mesh) {
        let c = doc.background.to_linear_premultiplied();
        let base = mesh.vertices.len() as u32;
        for p in [
            Vec2::ZERO,
            Vec2::new(doc.width, 0.0),
            doc.size(),
            Vec2::new(0.0, doc.height),
        ] {
            mesh.vertices.push([p.x, p.y, c[0], c[1], c[2], c[3]]);
        }
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Append the fills and strokes of `items` to `mesh`, back to front.
    /// Shapes that fail to tessellate are skipped and logged.
    pub fn draw_items(&mut self, items: &[DrawItem<'_>], mesh: &mut Mesh) {
        for item in items {
            let style = item.shape.style;
            if style.fill.is_none() && style.stroke.is_none() {
                continue;
            }
            self.build_path(&item.shape.path.cmds, item.transform);

            if let Some(fill) = style.fill {
                let c = color(fill, item.opacity);
                let opts = FillOptions::tolerance(self.tolerance).with_fill_rule(FillRule::NonZero);
                let result = self.fill.tessellate_path(
                    &self.path,
                    &opts,
                    &mut BuffersBuilder::new(mesh, |v: FillVertex| {
                        let p = v.position();
                        [p.x, p.y, c[0], c[1], c[2], c[3]]
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
                let opts = StrokeOptions::tolerance(self.tolerance).with_line_width(stroke.width * scale);
                let result = self.stroke.tessellate_path(
                    &self.path,
                    &opts,
                    &mut BuffersBuilder::new(mesh, |v: StrokeVertex| {
                        let p = v.position();
                        [p.x, p.y, c[0], c[1], c[2], c[3]]
                    }),
                );
                if let Err(e) = result {
                    log::warn!("stroke of \"{}\" failed: {e:?}", item.shape.name);
                }
            }
        }
    }

    /// Convert our commands to a lyon path in document space.
    fn build_path(&mut self, cmds: &[PathCmd], t: Affine2) {
        let p = |v: Vec2| {
            let w = t.transform_point2(v);
            point(w.x, w.y)
        };
        let mut b = LyonPath::builder();
        let mut open = false;
        for cmd in cmds {
            match *cmd {
                PathCmd::MoveTo(v) => {
                    if open {
                        b.end(false);
                    }
                    b.begin(p(v));
                    open = true;
                }
                PathCmd::LineTo(v) if open => {
                    b.line_to(p(v));
                }
                PathCmd::CubicTo(c1, c2, v) if open => {
                    b.cubic_bezier_to(p(c1), p(c2), p(v));
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

    #[test]
    fn opacity_premultiplies_colour() {
        let c = color(Color::WHITE, 0.5);
        assert_eq!(c, [0.5, 0.5, 0.5, 0.5]);
    }
}
