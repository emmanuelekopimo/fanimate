//! Apply the brand themes (brand/tokens/tokens.json) to egui.

use eframe::egui::epaint::text::{FontTweak, HintingTarget, SmoothHinting};
use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use serde::{Deserialize, Serialize};

const TOKENS: &str = include_str!("../../../brand/tokens/tokens.json");
const INTER: &[u8] = include_bytes!("../../../brand/fonts/Inter-Variable.ttf");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Night,
    Day,
    StudioGrey,
    HighContrast,
}

impl Theme {
    pub const ALL: [Theme; 4] = [Self::Night, Self::Day, Self::StudioGrey, Self::HighContrast];

    fn token_key(self) -> &'static str {
        match self {
            Self::Night => "night",
            Self::Day => "day",
            Self::StudioGrey => "studio-grey",
            Self::HighContrast => "high-contrast",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Night => "Fanimate Night",
            Self::Day => "Fanimate Day",
            Self::StudioGrey => "Studio Grey",
            Self::HighContrast => "High Contrast",
        }
    }
}

/// The theme's colours, read from the design tokens.
#[derive(Clone, Copy, Debug)]
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
    pub bone: Color32,
    pub success: Color32,
    pub error: Color32,
    pub dark: bool,
}

fn hex(s: &str) -> Color32 {
    let h = s.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    Color32::from_rgb(byte(0), byte(2), byte(4))
}

impl Palette {
    pub fn of(theme: Theme) -> Self {
        let tokens: serde_json::Value = serde_json::from_str(TOKENS).expect("tokens.json is valid JSON");
        let t = &tokens["themes"][theme.token_key()];
        let c = |k: &str| hex(t[k].as_str().unwrap_or("#FF00FF"));
        Self {
            pasteboard: c("pasteboard"),
            bg: c("bg"),
            panel: c("panel"),
            raised: c("raised"),
            border: c("border"),
            text: c("text"),
            text_muted: c("text-muted"),
            accent: c("accent"),
            accent_strong: c("accent-strong"),
            button: c("button"),
            on_button: c("on-button"),
            keyframe: c("keyframe"),
            bone: c("bone"),
            success: c("success"),
            error: c("error"),
            dark: theme != Theme::Day,
        }
    }
}

/// Inter for all text, with egui's bundled fonts kept as fallbacks for
/// symbols and icons.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // egui's defaults keep glyph shapes exact but look blurry at 100% scaling
    // (the 1366x768 laptops we design for): glyphs land on quarter pixels and
    // stems are never snapped sideways. Snap both to whole pixels instead.
    let crisp = FontTweak {
        subpixel_binning: Some(false),
        hinting_target: HintingTarget::Smooth(SmoothHinting {
            preserve_linear_metrics: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    fonts.font_data.insert(
        "Inter".into(),
        std::sync::Arc::new(FontData::from_static(INTER).tweak(crisp)),
    );
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "Inter".into());
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, theme: Theme) -> Palette {
    let p = Palette::of(theme);
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let radius = CornerRadius::same(4);

    v.override_text_color = Some(p.text);
    v.panel_fill = p.panel;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg;
    v.hyperlink_color = p.accent_strong;
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.accent_strong);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.warn_fg_color = p.keyframe;
    v.error_fg_color = p.error;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = p.border;
    w.hovered.weak_bg_fill = p.border;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = radius;

    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = p.accent;
    w.active.bg_stroke = Stroke::new(1.0, p.accent_strong);
    w.active.fg_stroke = Stroke::new(1.5, p.on_button);
    w.active.corner_radius = radius;

    w.open.bg_fill = p.border;
    w.open.weak_bg_fill = p.border;
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = radius;

    // the user's theme choice wins over the system's light/dark setting
    let egui_theme = if p.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.options_mut(|o| o.theme_preference = egui_theme.into());
    ctx.set_visuals_of(egui_theme, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.interaction.tooltip_delay = 0.4;
        // labels are interface text, not documents: no I-beam, no selection
        s.interaction.selectable_labels = false;
        s.interaction.multi_widget_text_select = false;
    });
    p
}

/// Style for every popup menu (menu bar and right-click menus). egui's own
/// menu style packs items at 2 px padding; desktop menus breathe more.
pub fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.item_spacing.y = 1.0;
    style.spacing.menu_margin = egui::Margin::same(4);
}
