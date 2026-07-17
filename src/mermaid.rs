use crate::layout::layout_flowchart;
use crate::parse::parse_graph;
use crate::sequence::{parse_sequence, layout_sequence};

/// Render a Mermaid diagram source string.
/// Returns the rendered text, or None if the input is empty/unsupported.
pub fn render(src: &str) -> Option<String> {
    if src.trim().is_empty() {
        return None;
    }

    // Try each parser in order
    if let Some(graph) = parse_graph(src) {
        return Some(layout_flowchart(&graph));
    }
    if let Some(seq) = parse_sequence(src) {
        return Some(layout_sequence(&seq));
    }

    // Fallback: wrap raw source in a box
    Some(fallback(src))
}

fn fallback(src: &str) -> String {
    let width = src.lines().map(|l| l.len()).max().unwrap_or(10).min(80) + 4;
    let top = format!("┌{}┐", "─".repeat(width - 2));
    let bottom = format!("└{}┘", "─".repeat(width - 2));
    let mut out = format!("{}\n", top);
    for line in src.lines().take(20) {
        let line = if line.len() > width - 4 {
            format!("{}…", &line[..width - 5])
        } else {
            line.to_string()
        };
        out.push_str(&format!("│ {:<width$} │\n", line, width = width - 4));
    }
    out.push_str(&bottom);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(render(""), None);
        assert_eq!(render("   "), None);
    }

    #[test]
    fn test_fallback_for_unsupported() {
        let result = render("pie title Test\n  \"A\": 50");
        assert!(result.is_some());
    }

    #[test]
    fn test_simple_flowchart() {
        let result = render("graph TD\n  A-->B");
        assert!(result.is_some());
    }
}
