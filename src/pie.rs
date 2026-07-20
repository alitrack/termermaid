//! Pie chart parser and renderer.
//!
//! Renders Mermaid pie charts as circular ASCII/ANSI art — not bar charts.
//! Uses a grid-based approach with terminal aspect-ratio correction (≈2:1).

use crate::theme;
use crate::theme::RESET;

const MAX_SLICES: usize = 32;
/// Terminal characters are roughly 2× taller than wide; scale Y for visual circularity.
const ASPECT: f64 = 2.0;
/// Radius in character-width units. The rendered circle will be 2R wide × R tall.
const RADIUS: usize = 10;

/// Fill characters for up to 8 slices (Unicode / ASCII).
const SECTOR_FILLS: [char; 8] = ['█', '▓', '▒', '░', '●', '◎', '◉', '○'];
const SECTOR_FILLS_ASCII: [char; 8] = ['#', '@', '%', '&', 'O', '0', '8', 'o'];

/// Border character for the circle edge.
const BORDER_CHAR: char = '·';
const BORDER_CHAR_ASCII: char = '.';

/// 256-color palette entries assigned to sectors — bright, visually distinct.
const SECTOR_COLORS_256: [u8; 8] = [196, 220, 46, 51, 33, 201, 129, 21];

pub struct PieChart {
    pub title: Option<String>,
    pub slices: Vec<PieSlice>,
}

pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

// ── Parser ────────────────────────────────────────────────────────────────

/// Parse a Mermaid pie chart.
pub fn parse_pie(src: &str) -> Option<PieChart> {
    let mut lines = src.lines().filter(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("%%")
    });
    let header = lines.next()?;
    let header_lower = header.trim().to_ascii_lowercase();
    if !header_lower.starts_with("pie") {
        return None;
    }

    let rest = header[3..].trim();
    let title = if rest.to_ascii_lowercase().starts_with("title ") {
        let t = rest[6..].trim().trim_matches('"').trim_matches('\'').trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    } else {
        None
    };

    let mut slices: Vec<PieSlice> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        if trimmed.to_ascii_lowercase() == "showdata" {
            continue;
        }
        if let Some((label_part, value_part)) = trimmed.split_once(':') {
            let label = label_part
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string();
            let value: f64 = value_part.trim().parse().ok()?;
            if value > 0.0 {
                slices.push(PieSlice { label, value });
                if slices.len() >= MAX_SLICES {
                    break;
                }
            }
        }
    }

    if slices.is_empty() {
        return None;
    }
    Some(PieChart { title, slices })
}

// ── Circular renderer ─────────────────────────────────────────────────────

/// Render a pie chart as a true filled circle, not horizontal bars.
///
/// Uses a grid where each character cell is evaluated for membership
/// in a visually-correct circle (2:1 aspect ratio) and assigned to
/// the appropriate sector based on angular position.
pub fn layout_pie(
    chart: &PieChart,
    ascii_only: bool,
    color_mode: theme::ColorMode,
    theme: &theme::Theme,
) -> String {
    let total: f64 = chart.slices.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        return String::new();
    }

    let use_color = color_mode != theme::ColorMode::None;
    let fills: &[char] = if ascii_only {
        &SECTOR_FILLS_ASCII
    } else {
        &SECTOR_FILLS
    };
    let border = if ascii_only {
        BORDER_CHAR_ASCII
    } else {
        BORDER_CHAR
    };

    let r = RADIUS;
    let diameter = 2 * r + 1; // odd width for a centre column
    let height = r + 1; // ≈ half the width (aspect-ratio correction)

    // Pre-compute sector boundaries in [0, 1).
    let mut sector_ends: Vec<f64> = Vec::with_capacity(chart.slices.len());
    let mut acc = 0.0;
    for s in &chart.slices {
        acc += s.value / total;
        sector_ends.push(acc);
    }

    let center_x = r as f64;
    let center_y = r as f64 / 2.0;
    let r_f = r as f64;

    let mut canvas: Vec<Vec<(char, Option<usize>)>> =
        vec![vec![(' ', None); diameter]; height];

    for y in 0..height {
        for x in 0..diameter {
            let dx = x as f64 - center_x;
            let dy = y as f64 - center_y;
            // Correct for terminal aspect ratio (chars ≈ 2× taller than wide).
            let dy_scaled = dy * ASPECT;
            let dist = (dx * dx + dy_scaled * dy_scaled).sqrt();

            if dist > r_f {
                continue;
            }

            // Edge detection: cells just inside the perimeter form the border.
            let on_edge = dist > r_f - 1.0;

            if on_edge {
                canvas[y][x] = (border, None);
            } else {
                // Angle: atan2 with aspect-corrected coords, then rotate so 12-o'clock = 0°.
                let angle = dy_scaled.atan2(dx); // [-π, π]
                let normalized = angle + std::f64::consts::FRAC_PI_2; // shift top → 0
                let normalized = if normalized < 0.0 {
                    normalized + 2.0 * std::f64::consts::PI
                } else if normalized >= 2.0 * std::f64::consts::PI {
                    normalized - 2.0 * std::f64::consts::PI
                } else {
                    normalized
                };
                let frac = normalized / (2.0 * std::f64::consts::PI);

                // Find the sector.
                for (i, end) in sector_ends.iter().enumerate() {
                    if frac < *end {
                        let idx = i % fills.len();
                        canvas[y][x] = (fills[idx], Some(i % SECTOR_COLORS_256.len()));
                        break;
                    }
                }
            }
        }
    }

    // ── Assemble output ──────────────────────────────────────────────────

    let mut out = String::new();

    // Title
    if let Some(ref title) = chart.title {
        out.push_str(title);
        out.push('\n');
        let hz = if ascii_only { '=' } else { '═' };
        for _ in 0..title.chars().count() {
            out.push(hz);
        }
        out.push('\n');
    }

    // Circle rows — apply ANSI colour per cell when enabled.
    for row in &canvas {
        let mut last_color: Option<u8> = None;
        for &(ch, color_idx) in row {
            if use_color {
                let current = color_idx.map(|i| SECTOR_COLORS_256[i]);
                if current != last_color {
                    // Close previous, open new.
                    if last_color.is_some() {
                        out.push_str(RESET);
                    }
                    if let Some(c) = current {
                        out.push_str(&bg256(c));
                    } else {
                        // No colour for space/border cells.
                    }
                    last_color = current;
                }
            }
            out.push(ch);
        }
        if use_color && last_color.is_some() {
            out.push_str(RESET);
        }
        out.push('\n');
    }

    // Legend
    for (i, slice) in chart.slices.iter().enumerate() {
        let pct = slice.value / total * 100.0;
        let fill = fills[i % fills.len()];
        if use_color {
            let color = SECTOR_COLORS_256[i % SECTOR_COLORS_256.len()];
            out.push_str(&format!(
                "  {}{}{} {} — {:.1}%\n",
                bg256(color),
                fill,
                RESET,
                slice.label,
                pct
            ));
        } else {
            out.push_str(&format!("  {} {} — {:.1}%\n", fill, slice.label, pct));
        }
    }

    out
}

/// Emit ANSI 256-colour background escape.
fn bg256(code: u8) -> String {
    format!("\x1b[48;5;{}m", code)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_theme() -> theme::Theme {
        theme::Theme::get(Default::default())
    }

    #[test]
    fn test_parse_simple() {
        let chart = parse_pie("pie title Test\n  \"Dogs\": 50\n  \"Cats\": 30").unwrap();
        assert_eq!(chart.title.as_deref(), Some("Test"));
        assert_eq!(chart.slices.len(), 2);
    }

    #[test]
    fn test_parse_no_title() {
        let chart = parse_pie("pie\n  \"A\": 10\n  \"B\": 20").unwrap();
        assert!(chart.title.is_none());
        assert_eq!(chart.slices.len(), 2);
    }

    #[test]
    fn test_parse_showdata() {
        let chart = parse_pie("pie showData\n  \"A\": 10").unwrap();
        assert_eq!(chart.slices.len(), 1);
    }

    #[test]
    fn test_render() {
        let chart = parse_pie("pie title Pets\n  \"Dogs\": 386\n  \"Cats\": 85").unwrap();
        let out = layout_pie(
            &chart,
            false,
            theme::ColorMode::None,
            &default_theme(),
        );
        assert!(out.contains("Pets"));
        assert!(out.contains("Dogs"));
        assert!(out.contains("Cats"));
        // The pie should be visibly circular — check for border char.
        assert!(out.contains(BORDER_CHAR));
    }

    #[test]
    fn test_render_ascii() {
        let chart = parse_pie("pie\n  \"A\": 10").unwrap();
        let out = layout_pie(&chart, true, theme::ColorMode::None, &default_theme());
        assert!(out.contains(BORDER_CHAR_ASCII));
        assert!(out.contains('#'));
    }

    #[test]
    fn test_render_circular_shape() {
        // Verify the output forms a roughly circular grid.
        let chart = parse_pie("pie\n  \"A\": 50\n  \"B\": 50").unwrap();
        let out = layout_pie(
            &chart,
            false,
            theme::ColorMode::None,
            &default_theme(),
        );
        let lines: Vec<&str> = out.lines().collect();
        // At least radius+1 lines of pie body.
        assert!(lines.len() >= RADIUS + 1, "too few lines: {}", lines.len());
        // First line of circle should have border chars.
        let first_line = lines.iter().find(|l| l.contains(BORDER_CHAR)).unwrap();
        assert!(!first_line.trim().is_empty());
    }

    #[test]
    fn test_many_slices() {
        let src = (0..8)
            .map(|i| format!("\"S{}\": {}", i, 100 - i * 10))
            .collect::<Vec<_>>()
            .join("\n");
        let chart = parse_pie(&format!("pie\n{}", src)).unwrap();
        let out = layout_pie(
            &chart,
            false,
            theme::ColorMode::None,
            &default_theme(),
        );
        // Should not panic.
        assert!(out.len() > 50);
    }
}
