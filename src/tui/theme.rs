// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use std::{path::Path, str::FromStr};

use ansi_colours::AsRGB;
use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};

/// Theme for UI components
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Theme {
    /// main background color
    pub background: Color,
    /// background color for selected items
    pub background_highlight: Color,
    /// border colors of componends
    pub border: Color,
    /// border color of focused component
    pub focus: Color,

    /// top panel color
    pub panel: Color,
    /// top panel shadow color
    pub panel_shadow: Color,

    /// base text color
    pub text: Color,
    /// text color for inactive content
    pub inactive_text: Color,
    /// color for highlighted text (shortcuts, titles, etc)
    pub highlight_text: Color,
    /// color for thinking blocks of model response
    pub think_block: Color,

    /// success message color
    pub success: Color,
    /// error message color and border color
    pub error: Color,

    /// assistant name color
    pub assistant: Color,
    /// user name color
    pub user: Color,
}

impl Theme {
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.background)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn border_style(&self, focused: bool) -> Style {
        Style::default()
            .fg(if focused { self.focus } else { self.border })
            .bg(self.background)
    }

    pub fn tool_result_style(&self, is_sucess: bool) -> Style {
        Style::default().fg(if is_sucess { self.success } else { self.error })
    }
}

struct Rgb(u8, u8, u8);

impl AsRGB for Rgb {
    fn as_u32(&self) -> u32 {
        (self.0 as u32) << 16 | (self.1 as u32) << 8 | self.2 as u32
    }
}

impl Theme {
    pub fn load(theme: impl AsRef<str>) -> Result<Self> {
        let is_256colors = std::env::var("TERM").is_ok_and(|t| t.contains("256"));
        let theme = theme.as_ref();
        let theme_source = if theme == "dark" {
            include_str!("../../themes/dark.yaml")
        } else if theme == "light" {
            include_str!("../../themes/light.yaml")
        } else {
            let path = Path::new(theme);
            if path.exists() {
                &std::fs::read_to_string(path)?
            } else {
                panic!("Theme file not found: {}", theme);
            }
        };
        let mut theme_val: Mapping = serde_yaml::from_str(&theme_source)?;
        if is_256colors {
            for (_, v) in theme_val.iter_mut() {
                if let Some(color) = v.as_str() {
                    let color = Color::from_str(color)?;
                    match color {
                        Color::Rgb(r, g, b) => {
                            let idx = ansi_colours::ansi256_from_rgb(Rgb(r, g, b));
                            *v = serde_yaml::to_value(Color::Indexed(idx))?;
                        }
                        _ => {}
                    }
                }
            }
        }
        let res_theme: Theme = serde_yaml::from_value(Value::Mapping(theme_val))?;
        Ok(res_theme)
    }
}
