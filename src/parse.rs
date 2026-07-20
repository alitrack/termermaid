//! Mermaid diagram parser.
//!
//! Ported from Grok Build `xai-grok-markdown/src/mermaid.rs` (Apache 2.0).

use crate::graph::{Dir, Edge, Graph, Group, Head, LineKind, Shape};
use std::collections::HashMap;

const MAX_GROUPS: usize = 24;
const MAX_GROUP_DEPTH: usize = 6;
const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 512;
const LABEL_BREAK_CHARS: [char; 4] = ['_', '-', '.', '/'];
const ENTITY_LOOKAHEAD: usize = 10;
const HTML_FORMAT_TAGS: &[&str] = &[
    "b", "strong", "i", "em", "u", "s", "strike", "del", "ins", "mark", "small", "big", "sub",
    "sup", "code", "kbd", "samp", "var", "tt", "span", "font", "q", "abbr", "cite", "pre",
];

// ─── Public API ─────────────────────────────────────────────

pub fn parse_graph(src: &str) -> Option<Graph> {
    let mut statements: Vec<String> = Vec::new();
    for raw_line in src.lines() {
        split_statements(raw_line, &mut statements);
    }

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

    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        index: HashMap::new(),
        groups: Vec::new(),
        node_group: Vec::new(),
        cur_group: None,
        over_cap: false,
        dir,
    };

    let mut stack: Vec<usize> = Vec::new();
    for st in &statements[1..] {
        let first_word = st.split_whitespace().next().unwrap_or("");
        match first_word.to_ascii_lowercase().as_str() {
            "subgraph" => {
                if graph.groups.len() >= MAX_GROUPS || stack.len() >= MAX_GROUP_DEPTH {
                    return None;
                }
                let (id, label) = parse_subgraph_decl(st["subgraph".len()..].trim());
                graph.groups.push(Group {
                    id,
                    label,
                    parent: stack.last().copied(),
                });
                stack.push(graph.groups.len() - 1);
                graph.cur_group = stack.last().copied();
                continue;
            }
            "end" => {
                stack.pop();
                graph.cur_group = stack.last().copied();
                continue;
            }
            "classdef" | "class" | "style" | "linkstyle" | "click" | "direction" => continue,
            _ => {}
        }
        parse_statement(st, &mut graph);
        if graph.over_cap {
            return None;
        }
    }

    if graph.nodes.is_empty() {
        return None;
    }
    Some(graph)
}

pub fn parse_state(src: &str) -> Option<Graph> {
    let mut statements: Vec<String> = Vec::new();
    for raw_line in src.lines() {
        split_statements(raw_line, &mut statements);
    }
    let header = statements.first()?;
    if !header
        .split_whitespace()
        .next()?
        .to_ascii_lowercase()
        .starts_with("statediagram")
    {
        return None;
    }

    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        index: HashMap::new(),
        groups: Vec::new(),
        node_group: Vec::new(),
        cur_group: None,
        over_cap: false,
        dir: Dir::Down,
    };

    let mut in_state = false;
    for st in &statements[1..] {
        let first_word = st.split_whitespace().next().unwrap_or("");
        match first_word.to_ascii_lowercase().as_str() {
            "state" => {
                in_state = true;
                parse_state_decl(st, &mut graph);
            }
            "note" => continue,
            _ => {
                if in_state {
                    parse_state_desc(st, &mut graph);
                } else {
                    parse_statement(st, &mut graph);
                }
            }
        }
        if graph.over_cap {
            return None;
        }
    }

    if graph.nodes.is_empty() {
        return None;
    }
    Some(graph)
}

pub fn parse_class(src: &str) -> Option<(Graph, Vec<ClassInfo>)> {
    let mut statements = split_lines(src);
    let header = statements.first()?;
    if !header
        .split_whitespace()
        .next()?
        .to_ascii_lowercase()
        .starts_with("classdiagram")
    {
        return None;
    }

    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        index: HashMap::new(),
        groups: Vec::new(),
        node_group: Vec::new(),
        cur_group: None,
        over_cap: false,
        dir: Dir::Down,
    };
    let mut infos: Vec<ClassInfo> = Vec::new();
    let mut merged = String::new();
    let mut in_class = false;

    for st in &statements[1..] {
        if st.is_empty() {
            continue;
        }
        // Accumulate multi-line class definitions
        if in_class {
            merged.push('\n');
            merged.push_str(st);
            if st.contains('}') {
                in_class = false;
                if let Some(ci) = parse_class_def(&merged) {
                    if let Some(idx) = graph.node_label(&ci.name, &ci.name) {
                        while infos.len() <= idx {
                            infos.push(ClassInfo::default());
                        }
                        infos[idx] = ci;
                    }
                }
                merged.clear();
            }
            continue;
        }
        if st.contains('{') && !st.contains('}') {
            merged = st.clone();
            in_class = true;
            continue;
        }
        if let Some(ci) = parse_class_def(st) {
            if let Some(idx) = graph.node_label(&ci.name, &ci.name) {
                while infos.len() <= idx {
                    infos.push(ClassInfo::default());
                }
                infos[idx] = ci;
            }
            continue;
        }
        if let Some(ann) = parse_class_annotation(st) {
            if let Some(idx) = graph.node_label(&ann.0, &ann.0) {
                while infos.len() <= idx {
                    infos.push(ClassInfo::default());
                }
                infos[idx].stereotype = Some(ann.1);
            }
            continue;
        }
        if let Some((from, to, hf, ht, line, label)) = parse_class_relation(st) {
            let fi = graph.node_label(&from, &from)?;
            let ti = graph.node_label(&to, &to)?;
            if graph.edges.len() < MAX_EDGES {
                graph.edges.push(Edge {
                    from: fi,
                    to: ti,
                    label,
                    head_from: hf,
                    head_to: ht,
                    line,
                });
            }
            continue;
        }
    }

    if graph.nodes.is_empty() {
        return None;
    }
    Some((graph, infos))
}

pub fn parse_er(src: &str) -> Option<(Graph, Vec<ClassInfo>)> {
    let mut statements = split_lines(src);
    let header = statements.first()?;
    if !header
        .split_whitespace()
        .next()?
        .eq_ignore_ascii_case("erdiagram")
    {
        return None;
    }

    let mut graph = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        index: HashMap::new(),
        groups: Vec::new(),
        node_group: Vec::new(),
        cur_group: None,
        over_cap: false,
        dir: Dir::Down,
    };
    let mut infos: Vec<ClassInfo> = Vec::new();
    let mut merged = String::new();
    let mut in_entity = false;

    for st in &statements[1..] {
        if st.is_empty() {
            continue;
        }
        // Accumulate multi-line entity definitions
        if in_entity {
            merged.push('\n');
            merged.push_str(st);
            if st.contains('}') {
                in_entity = false;
                if let Some(ci) = parse_er_entity(&merged) {
                    if let Some(idx) = graph.node_label(&ci.name, &ci.name) {
                        while infos.len() <= idx {
                            infos.push(ClassInfo::default());
                        }
                        infos[idx] = ci;
                    }
                }
                merged.clear();
            }
            continue;
        }
        // Try relation first (handles ||--o{ cardinality containing {)
        if let Some((from, to, card_from, card_to, line)) = parse_er_relation(st) {
            let fi = graph.node_label(&from, &from)?;
            let ti = graph.node_label(&to, &to)?;
            if graph.edges.len() < MAX_EDGES {
                let label = Some(format!("{} → {}", card_from, card_to));
                graph.edges.push(Edge {
                    from: fi,
                    to: ti,
                    label,
                    head_from: Head::None,
                    head_to: Head::None,
                    line,
                });
            }
            continue;
        }
        // Entity definition (possibly multi-line)
        if st.contains('{') && !st.contains('}') {
            merged = st.clone();
            in_entity = true;
            continue;
        }
        if st.contains('{') {
            if let Some(ci) = parse_er_entity(st) {
                if let Some(idx) = graph.node_label(&ci.name, &ci.name) {
                    while infos.len() <= idx {
                        infos.push(ClassInfo::default());
                    }
                    infos[idx] = ci;
                }
            }
            continue;
        }
    }

    if graph.nodes.is_empty() {
        return None;
    }
    Some((graph, infos))
}

// ─── Class Diagram ──────────────────────────────────────────

#[derive(Default, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub stereotype: Option<String>,
    pub members: Vec<String>,
    pub methods: Vec<String>,
}

fn parse_class_def(st: &str) -> Option<ClassInfo> {
    if !st.contains("class ") && !st.contains('{') {
        return None;
    }
    let parts: Vec<&str> = st.splitn(2, '{').collect();
    let decl = parts[0].trim();
    let body = parts.get(1).and_then(|b| b.strip_suffix('}')).unwrap_or("");
    let name = if decl.starts_with("class ") {
        decl["class ".len()..].trim().to_string()
    } else {
        decl.to_string()
    };
    if name.is_empty() {
        return None;
    }
    let mut ci = ClassInfo {
        name,
        ..Default::default()
    };
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains('(') || trimmed.contains("()") {
            ci.methods.push(trimmed.to_string());
        } else {
            ci.members.push(trimmed.to_string());
        }
    }
    Some(ci)
}

fn parse_class_annotation(st: &str) -> Option<(String, String)> {
    if !st.contains("<<") || !st.contains(">>") {
        return None;
    }
    let parts: Vec<&str> = st.split("<<").collect();
    let name = parts[0].trim().to_string();
    let stereo = parts[1].split(">>").next()?.trim().to_string();
    if name.is_empty() || stereo.is_empty() {
        return None;
    }
    Some((name, stereo))
}

fn parse_class_relation(
    st: &str,
) -> Option<(String, String, Head, Head, LineKind, Option<String>)> {
    const OPS: &[(&str, Head, Head, LineKind)] = &[
        ("<|--", Head::Triangle, Head::None, LineKind::Solid),
        ("--|>", Head::None, Head::Triangle, LineKind::Solid),
        ("*--", Head::None, Head::None, LineKind::Solid),
        ("--*", Head::None, Head::None, LineKind::Solid),
        ("o--", Head::Circle, Head::None, LineKind::Solid),
        ("--o", Head::None, Head::Circle, LineKind::Solid),
        ("<..", Head::Triangle, Head::None, LineKind::Dotted),
        ("..>", Head::None, Head::Triangle, LineKind::Dotted),
        ("-->", Head::None, Head::Arrow, LineKind::Solid),
        ("..>", Head::None, Head::Arrow, LineKind::Dotted),
    ];
    for &(op, hf, ht, line) in OPS {
        if let Some(pos) = st.find(op) {
            let from = st[..pos].trim().to_string();
            let to = st[pos + op.len()..]
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if from.is_empty() || to.is_empty() {
                continue;
            }
            let label = st.split(':').nth(1).map(|s| s.trim().to_string());
            return Some((from, to, hf, ht, line, label));
        }
    }
    None
}

// ─── ER Diagram ──────────────────────────────────────────────

fn parse_er_entity(st: &str) -> Option<ClassInfo> {
    let parts: Vec<&str> = st.splitn(2, '{').collect();
    let name = parts[0].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let mut ci = ClassInfo {
        name,
        ..Default::default()
    };
    if let Some(body) = parts.get(1).and_then(|b| b.strip_suffix('}')) {
        for line in body.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                ci.members.push(trimmed.to_string());
            }
        }
    }
    Some(ci)
}

fn parse_er_relation(st: &str) -> Option<(String, String, String, String, LineKind)> {
    let tokens: Vec<&str> = st.split_whitespace().collect();
    if tokens.len() < 3 {
        return None;
    }
    let from = tokens[0].to_string();
    let to = tokens[2].to_string();
    let (card_from, card_to, line) = parse_er_op(tokens[1])?;
    Some((from, to, card_from.to_string(), card_to.to_string(), line))
}

fn parse_er_op(tok: &str) -> Option<(&'static str, &'static str, LineKind)> {
    if !tok.is_ascii() || tok.len() != 6 {
        return None;
    }
    let line = match &tok[2..4] {
        "--" => LineKind::Solid,
        ".." => LineKind::Dotted,
        _ => return None,
    };
    Some((er_card(&tok[..2])?, er_card(&tok[4..6])?, line))
}

fn er_card(tok: &str) -> Option<&'static str> {
    match tok {
        "|o" | "o|" => Some("0..1"),
        "||" => Some("1"),
        "}o" | "o{" => Some("0..*"),
        "}|" | "|{" => Some("1..*"),
        _ => None,
    }
}

// ─── Flowchart Parsing ───────────────────────────────────────

fn parse_subgraph_decl(rest: &str) -> (String, String) {
    if let Some(q) = rest.strip_prefix('"') {
        if let Some((label, _)) = q.split_once('"') {
            return (label.to_string(), decode_html_entities(label));
        }
    }
    if let Some(open) = rest.find('[') {
        let id = rest[..open].trim();
        let label = rest[open + 1..].trim_end_matches(']').trim();
        let label = clean_label(label);
        if !id.is_empty() && !label.is_empty() {
            return (id.to_string(), label);
        }
    }
    (rest.to_string(), rest.to_string())
}

fn split_statements(line: &str, out: &mut Vec<String>) {
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                in_quotes = false;
            }
            cur.push(c);
        } else {
            match c {
                '"' => {
                    in_quotes = true;
                    cur.push(c);
                }
                '%' if chars.peek() == Some(&'%') => break,
                ';' => flush_statement(&mut cur, out),
                _ => cur.push(c),
            }
        }
    }
    flush_statement(&mut cur, out);
}

fn flush_statement(cur: &mut String, out: &mut Vec<String>) {
    let trimmed = cur.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    cur.clear();
}

fn split_lines(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        split_statements(line, &mut out);
    }
    out
}

fn parse_statement(st: &str, graph: &mut Graph) {
    let chars: Vec<char> = st.chars().collect();
    let mut i = 0;

    let Some((mut prev, ni)) = parse_node_group(&chars, i, graph) else {
        return;
    };
    i = ni;

    loop {
        i = skip_spaces(&chars, i);
        if i >= chars.len() {
            break;
        }
        let Some((left, right, line, label, ni)) = parse_link(&chars, i) else {
            break;
        };
        i = skip_spaces(&chars, ni);
        let Some((next, ni)) = parse_node_group(&chars, i, graph) else {
            break;
        };
        i = ni;

        if graph.edges.len() >= MAX_EDGES {
            graph.over_cap = true;
            return;
        }
        graph.edges.push(Edge {
            from: prev,
            to: next,
            label,
            head_from: left,
            head_to: right,
            line,
        });
        prev = next;
    }
}

fn parse_node_group(chars: &[char], start: usize, graph: &mut Graph) -> Option<(usize, usize)> {
    let i = skip_spaces(chars, start);
    if i >= chars.len() {
        return None;
    }

    // Check for bracket shapes first
    for &(open, close, shape) in &[
        ("[", "]", Shape::Rect),
        ("(", ")", Shape::Round),
        ("{", "}", Shape::Diamond),
        ("[\"", "\"]", Shape::Rect),
        ("(\"", "\")", Shape::Round),
        ("{", "}", Shape::Diamond),
        ("[/", "/]", Shape::Rect),
        ("[\\", "\\]", Shape::Rect),
        ("[(", ")]", Shape::Round),
        ("((", "))", Shape::Round),
        ("{{", "}}", Shape::Diamond),
    ] {
        let open_chars: Vec<char> = open.chars().collect();
        if chars[i..].starts_with(&open_chars) {
            let (_, label, ni) = extract_bracket(chars, i + open_chars.len(), close, shape);
            let label = label.unwrap_or_else(|| "?".to_string());
            let id = label.clone();
            let idx = graph.node_index(&id, Some(&label), shape)?;
            return Some((idx, ni));
        }
    }

    // Plain node ID (alphanumeric)
    let mut j = i;
    while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    if j > i {
        let id: String = chars[i..j].iter().collect();
        let idx = graph.node_index(&id, Some(&id), Shape::Round)?;
        return Some((idx, j));
    }

    // Quoted label
    if chars[i] == '"' {
        let mut j = i + 1;
        while j < chars.len() && chars[j] != '"' {
            j += 1;
        }
        if j < chars.len() {
            j += 1;
        }
        let label = decode_html_entities(&chars[i + 1..j - 1].iter().collect::<String>());
        let idx = graph.node_index(&label, Some(&label), Shape::Round)?;
        return Some((idx, j));
    }

    None
}

fn extract_bracket(
    chars: &[char],
    start: usize,
    closer: &str,
    shape: Shape,
) -> (Option<Shape>, Option<String>, usize) {
    let closer: Vec<char> = closer.chars().collect();
    let mut i = start;
    let mut text = String::new();
    let quoted = chars.get(start) == Some(&'"');
    let mut in_quotes = false;
    while i < chars.len() {
        let c = chars[i];
        if quoted && c == '"' {
            in_quotes = !in_quotes;
            text.push(c);
            i += 1;
            continue;
        }
        if !in_quotes && chars[i..].starts_with(closer.as_slice()) {
            let label = clean_label(&text);
            return (Some(shape), Some(label), i + closer.len());
        }
        text.push(c);
        i += 1;
    }
    (Some(shape), Some(clean_label(&text)), chars.len())
}

fn parse_link(
    chars: &[char],
    start: usize,
) -> Option<(Head, Head, LineKind, Option<String>, usize)> {
    let mut i = skip_spaces(chars, start);
    let mut left = Head::None;
    if let Some(&c) = chars.get(i) {
        if matches!(c, 'o' | 'x')
            && matches!(chars.get(i + 1), Some('-' | '.' | '='))
        {
            left = if c == 'o' { Head::Circle } else { Head::Cross };
            i += 1;
        }
    }
    let op_start = i;
    while i < chars.len() && matches!(chars[i], '-' | '.' | '=' | '<' | '>') {
        i += 1;
    }
    if i == op_start {
        return None;
    }
    let op1: String = chars[op_start..i].iter().collect();
    if left == Head::None && op1.starts_with('<') {
        left = Head::Arrow;
    }
    let mut line = line_kind(&op1);
    let mut right = if op1.contains('>') {
        Head::Arrow
    } else {
        Head::None
    };
    if right == Head::None {
        if let Some((head, ni)) = trailing_head(chars, i) {
            right = head;
            i = ni;
        }
    }

    // Label between pipes: A -- text --> B
    if chars.get(i) == Some(&'|') {
        i += 1;
        let l_start = i;
        while i < chars.len() && chars[i] != '|' {
            i += 1;
        }
        let label = clean_label(&chars[l_start..i].iter().collect::<String>());
        if chars.get(i) == Some(&'|') {
            i += 1;
        }
        return Some((left, right, line, non_empty(label), i));
    }

    // Text label before second arrow: A -- text --> B
    if right == Head::None {
        let text_start = skip_spaces(chars, i);
        let mut j = text_start;
        while j < chars.len() && !is_link_char(chars[j]) {
            j += 1;
        }
        if j < chars.len() && j > text_start && matches!(chars[j], '-' | '.' | '=' | '>') {
            let text: String = chars[text_start..j].iter().collect();
            let op2_start = j;
            while j < chars.len() && is_link_char(chars[j]) {
                j += 1;
            }
            let op2: String = chars[op2_start..j].iter().collect();
            right = if op2.contains('>') {
                Head::Arrow
            } else if let Some((head, nj)) = trailing_head(chars, j) {
                j = nj;
                head
            } else {
                Head::None
            };
            if line == LineKind::Solid {
                line = line_kind(&op2);
            }
            return Some((left, right, line, non_empty(clean_label(&text)), j));
        }
    }

    Some((left, right, line, None, i))
}

fn line_kind(op: &str) -> LineKind {
    if op.contains('=') {
        LineKind::Thick
    } else if op.contains('.') {
        LineKind::Dotted
    } else {
        LineKind::Solid
    }
}

fn trailing_head(chars: &[char], i: usize) -> Option<(Head, usize)> {
    let head = match chars.get(i) {
        Some('o') => Head::Circle,
        Some('x') => Head::Cross,
        _ => return None,
    };
    match chars.get(i + 1) {
        None | Some(' ') | Some('\t') | Some('|') | Some('&') | Some(';') => Some((head, i + 1)),
        _ => None,
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

fn is_link_char(c: char) -> bool {
    matches!(c, '-' | '.' | '=' | '<' | '>')
}

fn skip_spaces(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    i
}

// ─── State Diagram Parsing ───────────────────────────────────

fn parse_state_decl(st: &str, graph: &mut Graph) -> Option<()> {
    let rest = st["state".len()..].trim().trim_end_matches('{').trim();
    if rest.is_empty() {
        return Some(());
    }
    if let Some(q) = rest.strip_prefix('"') {
        let (label, after) = q.split_once('"')?;
        let id = after
            .trim()
            .strip_prefix("as")
            .map(str::trim)
            .unwrap_or(label);
        graph.node_label(id, &decode_html_entities(label))?;
        return Some(());
    }
    let id = rest;
    graph.node_label(id, id)?;
    Some(())
}

fn parse_state_desc(st: &str, graph: &mut Graph) -> Option<()> {
    if let Some((id, desc)) = st.split_once(':') {
        let id = id.trim();
        let desc = desc.trim();
        if id.is_empty() || id.contains(char::is_whitespace) || desc.is_empty() {
            return None;
        }
        graph.node_label(id, &decode_html_entities(desc))?;
    } else if !st.contains(char::is_whitespace) {
        graph.node_index(st, None, Shape::Round)?;
    } else {
        return None;
    }
    Some(())
}

// ─── Label Processing ────────────────────────────────────────

fn clean_label(raw: &str) -> String {
    let stripped = strip_html_tags(raw.trim());
    let trimmed = stripped.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| trimmed.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')))
        .unwrap_or(trimmed)
        .trim();
    let text = if let Some(md) = unquoted.strip_prefix('`').and_then(|t| t.strip_suffix('`')) {
        strip_markdown(md.trim())
    } else {
        unquoted.to_string()
    };
    decode_html_entities(&text)
}

fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let hi = (i + 1 + ENTITY_LOOKAHEAD).min(chars.len());
        let semi = (i + 1..hi).find(|&j| chars[j] == ';');
        let decoded = semi.and_then(|j| {
            let body: String = chars[i + 1..j].iter().collect();
            decode_entity_body(&body).map(|c| (c, j))
        });
        match decoded {
            Some((c, j)) => {
                out.push(c);
                i = j + 1;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

fn decode_entity_body(body: &str) -> Option<char> {
    match body {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = body.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse::<u32>().ok()?,
            };
            char::from_u32(code).filter(|c| !c.is_control())
        }
    }
}

fn strip_markdown(s: &str) -> String {
    let no_code: String = s.chars().filter(|&c| c != '`').collect();
    let no_strong = no_code.replace("**", "").replace("__", "");
    let chars: Vec<char> = no_strong.chars().collect();
    let mut out = String::with_capacity(no_strong.len());
    for (i, &c) in chars.iter().enumerate() {
        if (c == '*' || c == '_')
            && !(i > 0
                && chars[i - 1].is_alphanumeric()
                && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric()))
        {
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

fn strip_html_tags(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            if let Some((name, end)) = html_tag_at(&chars, i) {
                let lower = name.to_ascii_lowercase();
                if lower == "br" {
                    out.push(' ');
                    i = end;
                    continue;
                }
                if HTML_FORMAT_TAGS.contains(&lower.as_str()) {
                    i = end;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn html_tag_at(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start + 1;
    if chars.get(i) == Some(&'/') {
        i += 1;
    }
    let name_start = i;
    while i < chars.len() && chars[i].is_ascii_alphanumeric() {
        i += 1;
    }
    if i == name_start {
        return None;
    }
    let name: String = chars[name_start..i].iter().collect();
    while i < chars.len() && chars[i] != '>' {
        if chars[i] == '<' {
            return None;
        }
        i += 1;
    }
    if chars.get(i) == Some(&'>') {
        Some((name, i + 1))
    } else {
        None
    }
}
