//! Studio Grey from the brand tokens (brand/tokens/tokens.json), as in the
//! desktop app.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

const TOKENS: &str = include_str!("../../../brand/tokens/tokens.json");
const INTER: &[u8] = include_bytes!("../../../brand/fonts/Inter-Variable.ttf");

#[derive(Clone, Copy)]
pub struct Palette {
    pub pasteboard: Color32,
    pub bg: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_strong: Color32,
    pub button: Color32,
    pub on_button: Color32,
    pub keyframe: Color32,
    pub error: Color32,
}

fn hex(v: &serde_json::Value) -> Color32 {
    let s = v.as_str().unwrap_or("#FF00FF").trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("FF"), 16).unwrap_or(255);
    Color32::from_rgb(byte(0), byte(2), byte(4))
}

impl Palette {
    pub fn studio_grey() -> Self {
        let tokens: serde_json::Value = serde_json::from_str(TOKENS).unwrap_or_default();
        let t = &tokens["themes"]["studio-grey"];
        Self {
            pasteboard: hex(&t["pasteboard"]),
            bg: hex(&t["bg"]),
            panel: hex(&t["panel"]),
            raised: hex(&t["raised"]),
            border: hex(&t["border"]),
            text: hex(&t["text"]),
            text_muted: hex(&t["text-muted"]),
            accent: hex(&t["accent"]),
            accent_strong: hex(&t["accent-strong"]),
            button: hex(&t["button"]),
            on_button: hex(&t["on-button"]),
            keyframe: hex(&t["keyframe"]),
            error: hex(&t["error"]),
        }
    }
}

pub fn apply(ctx: &egui::Context) -> Palette {
    let p = Palette::studio_grey();
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("Inter".into(), std::sync::Arc::new(FontData::from_static(INTER)));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "Inter".into());
    }
    ctx.set_fonts(fonts);

    let mut v = Visuals::dark();
    let radius = CornerRadius::same(4);
    v.override_text_color = Some(p.text);
    v.panel_fill = p.panel;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.hyperlink_color = p.accent_strong;
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.accent_strong);
    v.warn_fg_color = p.keyframe;
    v.error_fg_color = p.error;
    let w = &mut v.widgets;
    for (state, fill) in [
        (&mut w.noninteractive, p.panel),
        (&mut w.inactive, p.raised),
        (&mut w.hovered, p.border),
        (&mut w.open, p.border),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.fg_stroke = Stroke::new(1.0, p.text);
        state.corner_radius = radius;
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = p.accent;
    w.active.bg_stroke = Stroke::new(1.0, p.accent_strong);
    w.active.fg_stroke = Stroke::new(1.5, p.on_button);
    w.active.corner_radius = radius;
    ctx.options_mut(|o| o.theme_preference = egui::Theme::Dark.into());
    ctx.set_visuals_of(egui::Theme::Dark, v);
    p
}
