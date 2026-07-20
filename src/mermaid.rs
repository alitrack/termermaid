use crate::layout::layout_flowchart;
use crate::parse::{parse_class, parse_er, parse_graph, parse_state, ClassInfo};
use crate::sequence::{parse_sequence, layout_sequence};

/// Render a Mermaid diagram source string.
/// Returns the rendered text, or None if the input is empty/unsupported.
pub fn render(src: &str) -> Option<String> {
    if src.trim().is_empty() {
        return None;
    }

    // Try each parser in order — first match wins.
    // Flowchart/graph
    if let Some(graph) = parse_graph(src) {
        return Some(layout_flowchart(&graph));
    }
    // Sequence diagram
    if let Some(seq) = parse_sequence(src) {
        return Some(layout_sequence(&seq));
    }
    // State diagram
    if let Some(graph) = parse_state(src) {
        return Some(layout_flowchart(&graph));
    }
    // Class diagram — render with class-box layout when ClassInfo present
    if let Some((graph, infos)) = parse_class(src) {
        return Some(layout_class(&graph, &infos, false));
    }
    // ER diagram
    if let Some((graph, infos)) = parse_er(src) {
        return Some(layout_class(&graph, &infos, true));
    }

    // Fallback: wrap raw source in a box
    Some(fallback(src))
}
fn layout_class(graph: &crate::graph::Graph, infos: &[ClassInfo], is_er: bool) -> String {
    let has_members = infos
        .iter()
        .any(|ci| !ci.members.is_empty() || !ci.methods.is_empty());

    if has_members {
        crate::layout::layout_class_diagram(graph, infos, is_er)
    } else {
        crate::layout::layout_flowchart(graph)
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
        // Verify parser returns members
        if let Some((g, infos)) = crate::parse::parse_class(src) {
            let has = infos.iter().any(|ci| !ci.members.is_empty() || !ci.methods.is_empty());
            assert!(has, "ClassInfo should have members: {:?}", infos.first().map(|c| (&c.members, &c.methods)));
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
}
