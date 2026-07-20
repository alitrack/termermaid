use crate::layout::layout_flowchart;
use crate::parse::{parse_class, parse_er, parse_graph, parse_state, ClassInfo};
use crate::sequence::{layout_sequence, parse_sequence};

/// Render options.
#[derive(Clone, Copy)]
pub struct RenderOptions {
    /// Use ASCII box-drawing characters instead of Unicode.
    pub ascii_only: bool,
    /// Output format: false = text, true = JSON.
    pub format_json: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            ascii_only: false,
            format_json: false,
        }
    }
}

/// Render a Mermaid diagram source string.
/// Returns the rendered text, or None if the input is empty/unsupported.
pub fn render(src: &str) -> Option<String> {
    render_with_opts(src, RenderOptions::default())
}

/// Render with custom options.
pub fn render_with_opts(src: &str, opts: RenderOptions) -> Option<String> {
    if src.trim().is_empty() {
        return None;
    }

    // Try each parser in order — first match wins.
    let result = if let Some(graph) = parse_graph(src) {
        Some((layout_flowchart(&graph, opts.ascii_only), None))
    } else if let Some(seq) = parse_sequence(src) {
        Some((layout_sequence(&seq, opts.ascii_only), None))
    } else if let Some(graph) = parse_state(src) {
        Some((layout_flowchart(&graph, opts.ascii_only), None))
    } else if let Some((graph, infos)) = parse_class(src) {
        Some((layout_class(&graph, &infos, false, opts.ascii_only), Some("class")))
    } else if let Some((graph, infos)) = parse_er(src) {
        Some((layout_class(&graph, &infos, true, opts.ascii_only), Some("er")))
    } else {
        if opts.format_json {
            return Some(r#"{"error":"unsupported diagram type","fallback":true}"#.to_string());
        }
        return Some(fallback(src));
    };

    if let Some((output, diag_type)) = result {
        if opts.format_json {
            return Some(format_json(output, diag_type));
        }
        Some(output)
    } else {
        None
    }
}

fn format_json(output: String, diag_type: Option<&str>) -> String {
    let width = output.lines().map(|l| l.len()).max().unwrap_or(0);
    let height = output.lines().count();
    let escaped = output.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    format!(
        r#"{{"diagram_type":"{}","width":{},"height":{},"text":"{}"}}"#,
        diag_type.unwrap_or("unknown"),
        width,
        height,
        escaped
    )
}

/// Render a class/ER diagram with member boxes.
fn layout_class(graph: &crate::graph::Graph, infos: &[ClassInfo], is_er: bool, ascii_only: bool) -> String {
    let has_members = infos
        .iter()
        .any(|ci| !ci.members.is_empty() || !ci.methods.is_empty());

    if has_members {
        crate::layout::layout_class_diagram(graph, infos, is_er, ascii_only)
    } else {
        crate::layout::layout_flowchart(graph, ascii_only)
    }
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

    #[test]
    fn test_state_diagram() {
        let result = render("stateDiagram-v2\n  [*] --> Idle\n  Idle --> Running : start");
        assert!(result.is_some());
    }

    #[test]
    fn test_class_diagram() {
        let src = "classDiagram\n  class Animal {\n    +String name\n    +int age\n    +eat() void\n  }\n  Animal <|-- Dog";
        if let Some((g, infos)) = crate::parse::parse_class(src) {
            let has = infos
                .iter()
                .any(|ci| !ci.members.is_empty() || !ci.methods.is_empty());
            assert!(
                has,
                "ClassInfo should have members: {:?}",
                infos.first().map(|c| (&c.members, &c.methods))
            );
        }
        let result = render(src);
        assert!(result.is_some());
    }

    #[test]
    fn test_er_diagram() {
        let result = render(
            "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  CUSTOMER {\n    string name\n    string email PK\n  }",
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_sequence_diagram() {
        let result = render("sequenceDiagram\n  Alice->>Bob: Hello");
        assert!(result.is_some());
    }

    #[test]
    fn test_ascii_mode() {
        let opts = RenderOptions { ascii_only: true, ..Default::default() };
        let result = render_with_opts("graph TD\n  A-->B", opts);
        let out = result.unwrap();
        assert!(out.contains('+') || out.contains('-'));
    }

    #[test]
    fn test_json_output() {
        let opts = RenderOptions { format_json: true, ..Default::default() };
        let result = render_with_opts("graph TD\n  A-->B", opts);
        let out = result.unwrap();
        assert!(out.contains("\"text\""));
        assert!(out.contains("\"diagram_type\""));
    }
}
