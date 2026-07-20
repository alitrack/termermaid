//! Terminal color themes for diagram rendering.
//!
//! Design: Color is opt-in, applied by role (node, edge, label) at render time.
//! The library is pure `(source, opts) -> bytes` — CLI owns env detection.

/// How much color the render should emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// No ANSI escapes — byte-for-byte identical to monochrome output.
    #[default]
    None,
    /// ANSI 256-color escape sequences.
    Ansi256,
    /// 24-bit TrueColor escape sequences.
    TrueColor,
}

impl ColorMode {
    /// Detect from CLI flag: "auto" uses TTY detection, "always" forces on, "never" off.
    pub fn from_flag(flag: &str, is_tty: bool) -> Self {
        match flag {
            "always" => ColorMode::Ansi256, // conservative default: 256-color
            "never" => ColorMode::None,
            _ => {
                // "auto" or anything else: enable only on TTY
                if is_tty && std::env::var("NO_COLOR").is_err() {
                    ColorMode::Ansi256
                } else {
                    ColorMode::None
                }
            }
        }
    }
}

/// RGB color triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to ANSI 256-color index.
    pub fn to_ansi256(&self) -> u8 {
        if self.r == self.g && self.g == self.b {
            let gray = self.r;
            if gray < 8 {
                return 16;
            }
            if gray > 248 {
                return 231;
            }
            return 232 + ((gray as u16 - 8) * 24 / 247) as u8;
        }
        let r = (self.r as f32 / 255.0 * 5.0) as u8;
        let g = (self.g as f32 / 255.0 * 5.0) as u8;
        let b = (self.b as f32 / 255.0 * 5.0) as u8;
        16 + r * 36 + g * 6 + b
    }

    /// Foreground SGR escape for the given color mode.
    /// Returns an empty string for `ColorMode::None`.
    pub fn fg(&self, mode: ColorMode) -> String {
        match mode {
            ColorMode::None => String::new(),
            ColorMode::Ansi256 => format!("\x1b[38;5;{}m", self.to_ansi256()),
            ColorMode::TrueColor => format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b),
        }
    }
}

/// ANSI reset sequence.
pub const RESET: &str = "\x1b[0m";

/// Theme for diagram rendering with role-based colors.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    /// Node text and border color.
    pub node_fg: Option<Color>,
    /// Edge connectors and arrow heads.
    pub edge: Option<Color>,
    /// Edge labels.
    pub edge_label: Option<Color>,
    /// Start/end markers (e.g. [*] in state diagrams).
    pub start_end: Option<Color>,
}

impl Theme {
    pub fn get(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Default => default_theme(),
            ThemeType::Terra => terra_theme(),
            ThemeType::Neon => neon_theme(),
            ThemeType::Mono => mono_theme(),
            ThemeType::Amber => amber_theme(),
            ThemeType::Phosphor => phosphor_theme(),
        }
    }
}

/// Named theme variants.
#[derive(Debug, Clone, Copy, Default)]
pub enum ThemeType {
    #[default]
    Default,
    Terra,
    Neon,
    Mono,
    Amber,
    Phosphor,
}

impl std::str::FromStr for ThemeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(ThemeType::Default),
            "terra" => Ok(ThemeType::Terra),
            "neon" => Ok(ThemeType::Neon),
            "mono" => Ok(ThemeType::Mono),
            "amber" => Ok(ThemeType::Amber),
            "phosphor" => Ok(ThemeType::Phosphor),
            _ => Ok(ThemeType::Default),
        }
    }
}

// ─── Theme definitions ────────────────────────────────────────

/// Default: every role inherits terminal colors — selecting this is a no-op.
fn default_theme() -> Theme {
    Theme {
        name: "default".into(),
        node_fg: None,
        edge: None,
        edge_label: None,
        start_end: None,
    }
}

/// Terra: warm earthy tones. Good on dark terminals.
fn terra_theme() -> Theme {
    Theme {
        name: "terra".into(),
        node_fg: Some(Color::new(255, 220, 180)),
        edge: Some(Color::new(255, 180, 100)),
        edge_label: Some(Color::new(255, 180, 100)),
        start_end: Some(Color::new(100, 80, 60)),
    }
}

/// Neon: bright synthwave colors for dark terminals.
fn neon_theme() -> Theme {
    Theme {
        name: "neon".into(),
        node_fg: Some(Color::new(255, 0, 255)),
        edge: Some(Color::new(0, 255, 127)),
        edge_label: Some(Color::new(0, 255, 255)),
        start_end: Some(Color::new(128, 0, 128)),
    }
}

/// Mono: high-contrast black and white.
fn mono_theme() -> Theme {
    Theme {
        name: "mono".into(),
        node_fg: Some(Color::new(255, 255, 255)),
        edge: Some(Color::new(192, 192, 192)),
        edge_label: Some(Color::new(192, 192, 192)),
        start_end: Some(Color::new(128, 128, 128)),
    }
}

/// Amber: classic terminal amber-on-black.
fn amber_theme() -> Theme {
    Theme {
        name: "amber".into(),
        node_fg: Some(Color::new(255, 192, 0)),
        edge: Some(Color::new(255, 128, 0)),
        edge_label: Some(Color::new(255, 192, 0)),
        start_end: Some(Color::new(128, 96, 0)),
    }
}

/// Phosphor: green phosphor CRT look.
fn phosphor_theme() -> Theme {
    Theme {
        name: "phosphor".into(),
        node_fg: Some(Color::new(0, 255, 0)),
        edge: Some(Color::new(0, 200, 0)),
        edge_label: Some(Color::new(0, 255, 0)),
        start_end: Some(Color::new(0, 128, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_noop() {
        let t = Theme::get(ThemeType::Default);
        assert!(t.node_fg.is_none());
        assert!(t.edge.is_none());
    }

    #[test]
    fn theme_has_colors() {
        let t = Theme::get(ThemeType::Neon);
        assert!(t.node_fg.is_some());
        assert!(t.edge.is_some());
    }

    #[test]
    fn ansi256_grayscale() {
        assert_eq!(Color::new(0, 0, 0).to_ansi256(), 16);
        assert_eq!(Color::new(255, 255, 255).to_ansi256(), 231);
    }

    #[test]
    fn color_mode_none_emits_empty() {
        assert_eq!(Color::new(255, 0, 0).fg(ColorMode::None), "");
    }

    #[test]
    fn theme_type_from_str() {
        assert_eq!("NeOn".parse::<ThemeType>().unwrap() as usize, ThemeType::Neon as usize);
        assert_eq!("terra".parse::<ThemeType>().unwrap() as usize, ThemeType::Terra as usize);
        assert_eq!("unknown".parse::<ThemeType>().unwrap() as usize, ThemeType::Default as usize);
    }

    #[test]
    fn color_mode_from_flag() {
        assert_eq!(ColorMode::from_flag("always", false) as usize, ColorMode::Ansi256 as usize);
        assert_eq!(ColorMode::from_flag("never", true) as usize, ColorMode::None as usize);
    }
}
