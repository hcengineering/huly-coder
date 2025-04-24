use ratatui::style::{Color, Style, Stylize};

/// Theme for UI components
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Background color for the application
    pub background: Color,
    /// Background color for odd rows
    pub background_highlight: Color,
    /// Text color for normal content
    pub text: Color,
    pub inactive_text: Color,
    pub tool_call: Color,
    /// Background color for toolbar panels
    pub panel: Color,
    pub panel_shadow: Color,
    /// Focuse border color
    pub focus: Color,
    /// Primary accent color
    pub primary: Color,
    /// Secondary accent color
    pub secondary: Color,
    /// Color for highlighting active elements
    pub highlight: Color,
    /// Color for success messages/indicators
    pub success: Color,
    /// Color for warning messages/indicators
    pub warning: Color,
    /// Color for error messages/indicators
    pub error: Color,
    /// Color for inactive or disabled elements
    pub inactive: Color,
    /// Status bar background color
    pub status: Color,
    /// Border color for widgets
    pub border: Color,
}

impl Theme {
    /// Create a new theme with custom colors
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        background: Color,
        background_highlight: Color,
        text: Color,
        inactive_text: Color,
        tool_call: Color,
        panel: Color,
        panel_shadow: Color,
        primary: Color,
        focus: Color,
        secondary: Color,
        highlight: Color,
        success: Color,
        warning: Color,
        error: Color,
        inactive: Color,
        status: Color,
        border: Color,
    ) -> Self {
        Self {
            background,
            background_highlight,
            text,
            inactive_text,
            tool_call,
            panel,
            panel_shadow,
            primary,
            focus,
            secondary,
            highlight,
            success,
            warning,
            error,
            inactive,
            status,
            border,
        }
    }

    /// Get the default dark theme
    pub fn dark() -> Self {
        Self {
            background: Color::from_u32(0x25262A),
            background_highlight: Color::from_u32(0x292A2E),
            panel: Color::from_u32(0x2A1F3A),
            panel_shadow: Color::from_u32(0x231A2F),
            text: Color::White,
            inactive_text: Color::from_u32(0x888888),
            tool_call: Color::from_u32(0x6EB4BF),
            focus: Color::from_u32(0x714AC9),
            primary: Color::from_u32(0xC1C9D5),
            secondary: Color::Blue,
            highlight: Color::Yellow,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            inactive: Color::DarkGray,
            status: Color::from_u32(0x303030),
            border: Color::from_u32(0x37383B),
        }
    }

    /// Get the default light theme
    pub fn light() -> Self {
        Self {
            background: Color::White,
            background_highlight: Color::from_u32(0xF0F0F0),
            panel: Color::LightMagenta,
            panel_shadow: Color::LightMagenta,
            text: Color::Black,
            inactive_text: Color::Black,
            tool_call: Color::Black,
            primary: Color::Blue,
            focus: Color::Blue,
            secondary: Color::Cyan,
            highlight: Color::Magenta,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            inactive: Color::Gray,
            status: Color::Gray,
            border: Color::DarkGray,
        }
    }

    pub fn panel_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.panel)
    }

    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.background)
    }

    pub fn primary_style(&self) -> Style {
        Style::default().fg(self.primary).bg(self.background).bold()
    }

    pub fn secondary_style(&self) -> Style {
        Style::default().fg(self.secondary).bg(self.background)
    }

    pub fn highlight_style(&self) -> Style {
        Style::default().fg(self.highlight)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn inactive_style(&self) -> Style {
        Style::default().fg(self.inactive)
    }

    pub fn inactive_text_style(&self) -> Style {
        Style::default().fg(self.inactive_text)
    }

    pub fn border_style(&self, focused: bool) -> Style {
        Style::default()
            .fg(if focused { self.focus } else { self.border })
            .bg(self.background)
    }

    pub fn tool_call_style(&self) -> Style {
        Style::default().fg(self.tool_call)
    }

    pub fn tool_result_style(&self, is_sucess: bool) -> Style {
        Style::default().fg(if is_sucess { self.success } else { self.error })
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
