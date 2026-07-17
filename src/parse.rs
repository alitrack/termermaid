use crate::graph::{Dir, Graph};

/// Split Mermaid source into statements, handling `;` separators.
pub fn split_statements(src: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    for raw_line in src.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("---") {
            continue;
        }
        for part in line.split(';') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
        }
    }
    statements
}

pub fn parse_graph(src: &str) -> Option<Graph> {
    let statements = split_statements(src);
    let header = statements.first()?;
    let mut header_tokens = header.split_whitespace();
    let kind = header_tokens.next()?.to_ascii_lowercase();
    if kind != "graph" && kind != "flowchart" {
        return None;
    }
    let dir = match header_tokens
        .next()
        .unwrap_or("TB")
        .to_ascii_uppercase()
        .as_str()
    {
        "LR" => Dir::Right,
        "RL" => Dir::Left,
        "BT" => Dir::Up,
        _ => Dir::Down,
    };

    let graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        index: std::collections::HashMap::new(),
        groups: Vec::new(),
        node_group: Vec::new(),
        cur_group: None,
        over_cap: false,
        dir,
    };

    let _ = &statements;

    if graph.nodes.is_empty() {
        return None;
    }
    Some(graph)
}
