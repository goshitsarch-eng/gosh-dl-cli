use ratatui::style::{Color, Modifier, Style};

/// Palette-based theme using Catppuccin color system.
/// Widgets pick from abstract color slots rather than role-specific fields.
/// All palette colors are kept even if not yet used by the UI.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Theme {
    // Background layers (depth)
    pub bg: Color,
    pub bg_dim: Color,
    pub bg_deep: Color,

    // Surface layers (interactive elements)
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,

    // Text hierarchy
    pub text: Color,
    pub subtext1: Color,
    pub subtext0: Color,
    pub overlay1: Color,
    pub overlay0: Color,

    // Semantic colors
    pub accent: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,

    // Extended palette
    pub pink: Color,
    pub mauve: Color,
    pub peach: Color,
    pub teal: Color,
    pub sky: Color,
    pub lavender: Color,
    pub flamingo: Color,
    pub rosewater: Color,
}

#[allow(dead_code)]
impl Theme {
    /// Catppuccin Mocha — default dark theme
    pub fn mocha() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),
            bg_dim: Color::Rgb(24, 24, 37),
            bg_deep: Color::Rgb(17, 17, 27),
            surface0: Color::Rgb(49, 50, 68),
            surface1: Color::Rgb(69, 71, 90),
            surface2: Color::Rgb(88, 91, 112),
            text: Color::Rgb(205, 214, 244),
            subtext1: Color::Rgb(186, 194, 222),
            subtext0: Color::Rgb(166, 173, 200),
            overlay1: Color::Rgb(127, 132, 156),
            overlay0: Color::Rgb(108, 112, 134),
            accent: Color::Rgb(137, 180, 250),
            success: Color::Rgb(166, 227, 161),
            error: Color::Rgb(243, 139, 168),
            warning: Color::Rgb(249, 226, 175),
            info: Color::Rgb(116, 199, 236),
            pink: Color::Rgb(245, 194, 231),
            mauve: Color::Rgb(203, 166, 247),
            peach: Color::Rgb(250, 179, 135),
            teal: Color::Rgb(148, 226, 213),
            sky: Color::Rgb(137, 220, 235),
            lavender: Color::Rgb(180, 190, 254),
            flamingo: Color::Rgb(242, 205, 205),
            rosewater: Color::Rgb(245, 224, 220),
        }
    }

    /// Catppuccin Macchiato — alternative dark theme
    pub fn macchiato() -> Self {
        Self {
            bg: Color::Rgb(36, 39, 58),
            bg_dim: Color::Rgb(30, 32, 48),
            bg_deep: Color::Rgb(24, 25, 38),
            surface0: Color::Rgb(54, 58, 79),
            surface1: Color::Rgb(73, 77, 100),
            surface2: Color::Rgb(91, 96, 120),
            text: Color::Rgb(202, 211, 245),
            subtext1: Color::Rgb(184, 192, 224),
            subtext0: Color::Rgb(165, 173, 203),
            overlay1: Color::Rgb(128, 135, 162),
            overlay0: Color::Rgb(110, 115, 141),
            accent: Color::Rgb(138, 173, 244),
            success: Color::Rgb(166, 218, 149),
            error: Color::Rgb(237, 135, 150),
            warning: Color::Rgb(238, 212, 159),
            info: Color::Rgb(125, 196, 228),
            pink: Color::Rgb(245, 189, 230),
            mauve: Color::Rgb(198, 160, 246),
            peach: Color::Rgb(245, 169, 127),
            teal: Color::Rgb(139, 213, 202),
            sky: Color::Rgb(145, 215, 227),
            lavender: Color::Rgb(183, 189, 248),
            flamingo: Color::Rgb(240, 198, 198),
            rosewater: Color::Rgb(244, 219, 214),
        }
    }

    /// Catppuccin Latte — light theme
    pub fn latte() -> Self {
        Self {
            bg: Color::Rgb(239, 241, 245),
            bg_dim: Color::Rgb(230, 233, 239),
            bg_deep: Color::Rgb(220, 224, 232),
            surface0: Color::Rgb(204, 208, 218),
            surface1: Color::Rgb(188, 192, 204),
            surface2: Color::Rgb(172, 176, 190),
            text: Color::Rgb(76, 79, 105),
            subtext1: Color::Rgb(92, 95, 119),
            subtext0: Color::Rgb(108, 111, 133),
            overlay1: Color::Rgb(140, 143, 161),
            overlay0: Color::Rgb(156, 160, 176),
            accent: Color::Rgb(30, 102, 245),
            success: Color::Rgb(64, 160, 43),
            error: Color::Rgb(210, 15, 57),
            warning: Color::Rgb(223, 142, 29),
            info: Color::Rgb(32, 159, 181),
            pink: Color::Rgb(234, 118, 203),
            mauve: Color::Rgb(136, 57, 239),
            peach: Color::Rgb(254, 100, 11),
            teal: Color::Rgb(23, 146, 153),
            sky: Color::Rgb(4, 165, 229),
            lavender: Color::Rgb(114, 135, 253),
            flamingo: Color::Rgb(221, 120, 120),
            rosewater: Color::Rgb(220, 138, 120),
        }
    }

    /// No-color fallback
    pub fn plain() -> Self {
        Self {
            bg: Color::Reset,
            bg_dim: Color::Reset,
            bg_deep: Color::Reset,
            surface0: Color::Reset,
            surface1: Color::Reset,
            surface2: Color::Reset,
            text: Color::Reset,
            subtext1: Color::Reset,
            subtext0: Color::Reset,
            overlay1: Color::Reset,
            overlay0: Color::Reset,
            accent: Color::Reset,
            success: Color::Reset,
            error: Color::Reset,
            warning: Color::Reset,
            info: Color::Reset,
            pink: Color::Reset,
            mauve: Color::Reset,
            peach: Color::Reset,
            teal: Color::Reset,
            sky: Color::Reset,
            lavender: Color::Reset,
            flamingo: Color::Reset,
            rosewater: Color::Reset,
        }
    }

    pub fn from_name(name: &str) -> Self {
        if !crate::format::color_enabled() {
            return Self::plain();
        }
        match name.to_lowercase().as_str() {
            "light" | "latte" => Self::latte(),
            "macchiato" => Self::macchiato(),
            _ => Self::mocha(),
        }
    }

    // ── Style helpers ──────────────────────────────────────────

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.overlay0)
    }

    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.surface1)
    }

    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Progress bar color based on completion percentage
    pub fn progress_color(&self, percent: f64) -> Color {
        if percent >= 100.0 {
            self.success
        } else if percent >= 70.0 {
            self.accent
        } else if percent >= 30.0 {
            self.peach
        } else {
            self.error
        }
    }

    pub fn lerp_color(a: Color, b: Color, t: f64) -> Color {
        if let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) {
            let t = t.clamp(0.0, 1.0);
            Color::Rgb(
                (r1 as f64 + (r2 as f64 - r1 as f64) * t) as u8,
                (g1 as f64 + (g2 as f64 - g1 as f64) * t) as u8,
                (b1 as f64 + (b2 as f64 - b1 as f64) * t) as u8,
            )
        } else if t < 0.5 {
            a
        } else {
            b
        }
    }

    pub fn progress_gradient(&self, t: f64) -> Color {
        // Three-stop: error -> peach -> success
        if t < 0.5 {
            Self::lerp_color(self.error, self.peach, t * 2.0)
        } else {
            Self::lerp_color(self.peach, self.success, (t - 0.5) * 2.0)
        }
    }

    pub fn dl_graph_gradient(&self, t: f64) -> Color {
        Self::lerp_color(self.mauve, self.teal, t)
    }

    pub fn ul_graph_gradient(&self, t: f64) -> Color {
        Self::lerp_color(self.peach, self.pink, t)
    }

    /// State-specific foreground color
    pub fn state_color(&self, state: &gosh_dl::DownloadState) -> Color {
        match state {
            gosh_dl::DownloadState::Downloading => self.pink,
            gosh_dl::DownloadState::Seeding => self.teal,
            gosh_dl::DownloadState::Paused => self.warning,
            gosh_dl::DownloadState::Queued => self.overlay1,
            gosh_dl::DownloadState::Connecting => self.sky,
            gosh_dl::DownloadState::Completed => self.success,
            gosh_dl::DownloadState::Error { .. } => self.error,
        }
    }
}
