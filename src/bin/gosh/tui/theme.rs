use ratatui::style::{Color, Modifier, Style};

#[derive(Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub header_bg: Color,
    pub header_fg: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub normal_fg: Color,
    pub muted_fg: Color,
    pub success_fg: Color,
    pub error_fg: Color,
    pub warning_fg: Color,
    pub progress_bar_filled: Color,
    pub progress_bar_empty: Color,
    pub border: Color,
    pub title_fg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            header_bg: Color::Rgb(30, 30, 40),
            header_fg: Color::White,
            selected_bg: Color::Rgb(50, 50, 70),
            selected_fg: Color::White,
            normal_fg: Color::Gray,
            muted_fg: Color::DarkGray,
            success_fg: Color::Green,
            error_fg: Color::Red,
            warning_fg: Color::Yellow,
            progress_bar_filled: Color::Cyan,
            progress_bar_empty: Color::DarkGray,
            border: Color::DarkGray,
            title_fg: Color::Cyan,
        }
    }

    pub fn light() -> Self {
        Self {
            header_bg: Color::Rgb(240, 240, 240),
            header_fg: Color::Black,
            selected_bg: Color::Rgb(200, 220, 255),
            selected_fg: Color::Black,
            normal_fg: Color::Black,
            muted_fg: Color::DarkGray,
            success_fg: Color::Rgb(0, 128, 0),
            error_fg: Color::Rgb(200, 0, 0),
            warning_fg: Color::Rgb(200, 150, 0),
            progress_bar_filled: Color::Blue,
            progress_bar_empty: Color::LightBlue,
            border: Color::Gray,
            title_fg: Color::Blue,
        }
    }

    pub fn plain() -> Self {
        Self {
            header_bg: Color::Reset,
            header_fg: Color::Reset,
            selected_bg: Color::Reset,
            selected_fg: Color::Reset,
            normal_fg: Color::Reset,
            muted_fg: Color::Reset,
            success_fg: Color::Reset,
            error_fg: Color::Reset,
            warning_fg: Color::Reset,
            progress_bar_filled: Color::Reset,
            progress_bar_empty: Color::Reset,
            border: Color::Reset,
            title_fg: Color::Reset,
        }
    }

    pub fn from_name(name: &str) -> Self {
        if !crate::format::color_enabled() {
            return Self::plain();
        }
        match name.to_lowercase().as_str() {
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }

    // Style helpers
    pub fn header_style(&self) -> Style {
        Style::default()
            .bg(self.header_bg)
            .fg(self.header_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().bg(self.selected_bg).fg(self.selected_fg)
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.normal_fg)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted_fg)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success_fg)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error_fg)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning_fg)
    }

    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.title_fg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }
}
