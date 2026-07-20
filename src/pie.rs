//! Pie chart parser and renderer.
//!
//! Renders Mermaid pie charts as labeled horizontal bars with percentages.

use crate::canvas::char_width;

const MAX_SLICES: usize = 32;
const BAR_WIDTH: usize = 20;

pub struct PieChart {
    pub title: Option<String>,
    pub slices: Vec<PieSlice>,
}

pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

/// Parse a Mermaid pie chart.
pub fn parse_pie(src: &str) -> Option<PieChart> {
    let mut lines = src.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("%%"));
    let header = lines.next()?;
    let header_lower = header.trim().to_ascii_lowercase();
    if !header_lower.starts_with("pie") {
        return None;
    }

    let rest = header[3..].trim();
    let title = if rest.to_ascii_lowercase().starts_with("title ") {
        let t = rest[6..].trim();
        // Strip quotes
        let t = t.trim_matches('"').trim_matches('\'').trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
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
        // Format: "Label" : value
        // or: Label : value (no quotes)
        if let Some((label_part, value_part)) = trimmed.split_once(':') {
            let label = label_part.trim().trim_matches('"').trim_matches('\'').trim().to_string();
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

/// Render a pie chart as a horizontal bar chart.
pub fn layout_pie(chart: &PieChart, ascii_only: bool, color_mode: crate::theme::ColorMode, theme: &crate::theme::Theme) -> String {
    let bar_char = if ascii_only { '#' } else { '█' };
    let total: f64 = chart.slices.iter().map(|s| s.value).sum();
    if total <= 0.0 {
        return String::new();
    }

    let use_color = color_mode != crate::theme::ColorMode::None;
    let color_on = use_color && theme.node_fg.is_some();
    let node_fg = theme.node_fg.as_ref().map(|c| c.fg(color_mode)).unwrap_or_default();
    let reset = if use_color { crate::theme::RESET } else { "" };

    let max_label_w = chart.slices.iter()
        .map(|s| s.label.chars().map(char_width).sum::<usize>())
        .max()
        .unwrap_or(0)
        .min(30);

    let mut out = String::new();

    // Title
    if let Some(ref title) = chart.title {
        out.push_str(title);
        out.push('\n');
        let hz = if ascii_only { '=' } else { '═' };
        let tw = title.chars().map(char_width).sum::<usize>();
        for _ in 0..tw {
            out.push(hz);
        }
        out.push('\n');
    }

    // Slices
    for slice in &chart.slices {
        let pct = (slice.value / total * 100.0).round() as usize;
        let bar_len = (slice.value / total * BAR_WIDTH as f64).round() as usize;

        // Label
        out.push_str("  ");
        out.push_str(&slice.label);
        for _ in slice.label.chars().map(char_width).sum::<usize>()..max_label_w {
            out.push(' ');
        }
        out.push(' ');

        // Bar
        for _ in 0..bar_len {
            out.push(bar_char);
        }
        out.push(' ');

        // Value + percentage
        out.push_str(&format!("{}", slice.value as u64));
        if slice.value.fract() > 0.0 {
            out.push_str(&format!(" ({:.01}%)", slice.value / total * 100.0));
        } else {
            out.push_str(&format!(" ({:.0}%)", pct));
        }
        out.push('\n');
    }

    if color_on {
        format!("{}{}{}", node_fg, out, reset)
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let out = layout_pie(&chart, false, crate::theme::ColorMode::None, &crate::theme::Theme::get(Default::default()));
        assert!(out.contains("Pets"));
        assert!(out.contains("Dogs"));
        assert!(out.contains("Cats"));
    }

    #[test]
    fn test_render_ascii() {
        let chart = parse_pie("pie\n  \"A\": 10").unwrap();
        let out = layout_pie(&chart, true, crate::theme::ColorMode::None, &crate::theme::Theme::get(Default::default()));
        assert!(out.contains('#'));
    }
}
