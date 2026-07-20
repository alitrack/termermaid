//! Sequence diagram parser and layout.
//!
//! Ported from Grok Build `xai-grok-markdown/src/mermaid.rs` (Apache 2.0).

use std::collections::HashMap;

use crate::canvas::{char_width, fit_label, PAD, WRAP_WIDTH};
use crate::graph::Shape;
use crate::layout::{Canvas, Placed};

const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 512;
const MAX_CANVAS_CELLS: usize = 1 << 21;
const SEQ_GAP: usize = 5;

// ─── Sequence Data Types ─────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SeqHead {
    Arrow,
    Cross,
}

enum NoteAnchor {
    Over(usize, usize),
    Left(usize),
    Right(usize),
}

enum SeqItem {
    Message {
        from: usize,
        to: usize,
        text: Option<String>,
        dashed: bool,
        head: SeqHead,
    },
    Note {
        anchor: NoteAnchor,
        text: String,
    },
    Divider {
        text: String,
    },
}

pub struct Sequence {
    labels: Vec<String>,
    index: HashMap<String, usize>,
    items: Vec<SeqItem>,
}

impl Sequence {
    fn participant(&mut self, id: &str, label: Option<&str>) -> Option<usize> {
        if let Some(&i) = self.index.get(id) {
            if let Some(label) = label {
                self.labels[i] = label.to_string();
            }
            return Some(i);
        }
        if self.labels.len() >= MAX_NODES {
            return None;
        }
        self.index.insert(id.to_string(), self.labels.len());
        self.labels.push(label.unwrap_or(id).to_string());
        Some(self.labels.len() - 1)
    }
}

const SEQ_OPS: &[(&str, bool, SeqHead)] = &[
    ("-->>", true, SeqHead::Arrow),
    ("->>", false, SeqHead::Arrow),
    ("--x", true, SeqHead::Cross),
    ("-x", false, SeqHead::Cross),
    ("--)", true, SeqHead::Arrow),
    ("-)", false, SeqHead::Arrow),
    ("-->", true, SeqHead::Arrow),
    ("->", false, SeqHead::Arrow),
];

// ─── Parser ──────────────────────────────────────────────────

/// Clean a label: strip HTML tags, quotes, markdown, decode entities.
fn clean_label(s: &str) -> String {
    let mut s = s.to_string();
    // Strip HTML tags
    let html_tags = &[
        "b", "strong", "i", "em", "u", "s", "strike", "del", "ins", "mark",
        "small", "big", "sub", "sup", "code", "kbd", "samp", "var", "tt",
        "span", "font", "q", "abbr", "cite", "pre",
    ];
    for tag in html_tags {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        s = s.replace(&open, "").replace(&close, "");
    }
    // Strip quotes
    s = s.trim_matches(&['"', '\'', '`'][..]).to_string();
    // Decode basic HTML entities
    s = s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#xa;", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n");
    if s.is_empty() { "?".to_string() } else { s }
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

pub fn parse_sequence(src: &str) -> Option<Sequence> {
    let mut statements: Vec<String> = Vec::new();
    for raw_line in src.lines() {
        // Split on semicolons
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        for part in trimmed.split(';') {
            let s = part.trim().to_string();
            if !s.is_empty() {
                statements.push(s);
            }
        }
    }
    let header = statements.first()?;
    if !header
        .split_whitespace()
        .next()?
        .eq_ignore_ascii_case("sequencediagram")
    {
        return None;
    }

    let mut seq = Sequence {
        labels: Vec::new(),
        index: HashMap::new(),
        items: Vec::new(),
    };
    let mut autonumber = false;
    let mut msg_count = 0usize;
    let mut blocks: Vec<bool> = Vec::new();

    for st in &statements[1..] {
        let first = st.split_whitespace().next().unwrap_or("");
        match first.to_ascii_lowercase().as_str() {
            "participant" | "actor" => {
                let rest = st[first.len()..].trim();
                if rest.is_empty() {
                    return None;
                }
                let (id, label) = match rest.split_once(" as ") {
                    Some((id, label)) => (id.trim(), Some(clean_label(label))),
                    None => (rest, None),
                };
                seq.participant(id, label.as_deref())?;
            }
            "autonumber" => autonumber = true,
            "activate" | "deactivate" | "create" | "destroy" | "title" | "acctitle"
            | "accdescr" | "links" | "link" | "properties" => {}
            "note" => {
                let rest = st[first.len()..].trim();
                let (text_part, anchor) = parse_note_anchor(rest, &mut seq)?;
                if seq.items.len() >= MAX_EDGES {
                    return None;
                }
                seq.items.push(SeqItem::Note {
                    anchor,
                    text: text_part,
                });
            }
            "loop" | "alt" | "opt" | "par" | "critical" | "break" | "else" | "and" | "option" => {
                if matches!(
                    first.to_ascii_lowercase().as_str(),
                    "else" | "and" | "option"
                ) {
                    if blocks.last() != Some(&true) {
                        continue;
                    }
                } else {
                    blocks.push(true);
                }
                if seq.items.len() >= MAX_EDGES {
                    return None;
                }
                seq.items.push(SeqItem::Divider {
                    text: decode_html_entities(st),
                });
            }
            "rect" | "box" => blocks.push(false),
            "end" => {
                if blocks.pop() == Some(true) {
                    if seq.items.len() >= MAX_EDGES {
                        return None;
                    }
                    seq.items.push(SeqItem::Divider {
                        text: "end".to_string(),
                    });
                }
            }
            _ => {
                let (from, to, mut text, dashed, head) = parse_seq_message(st, &mut seq)?;
                if autonumber {
                    msg_count += 1;
                    text = Some(match text {
                        Some(t) => format!("{}. {}", msg_count, t),
                        None => format!("{}.", msg_count),
                    });
                }
                if seq.items.len() >= MAX_EDGES {
                    return None;
                }
                seq.items.push(SeqItem::Message {
                    from,
                    to,
                    text,
                    dashed,
                    head,
                });
            }
        }
    }

    if seq.labels.is_empty() {
        return None;
    }
    Some(seq)
}

fn parse_note_anchor(rest: &str, seq: &mut Sequence) -> Option<(String, NoteAnchor)> {
    let lower = rest.to_ascii_lowercase();
    let (ids_and_text, kind) = if let Some(r) = lower.strip_prefix("over ") {
        (&rest[rest.len() - r.len()..], 0u8)
    } else if let Some(r) = lower.strip_prefix("left of ") {
        (&rest[rest.len() - r.len()..], 1)
    } else if let Some(r) = lower.strip_prefix("right of ") {
        (&rest[rest.len() - r.len()..], 2)
    } else {
        return None;
    };
    let (ids, text) = ids_and_text.split_once(':')?;
    let text = decode_html_entities(text.trim());
    let mut parts = ids.split(',').map(str::trim).filter(|s| !s.is_empty());
    let a = seq.participant(parts.next()?, None)?;
    let anchor = match kind {
        0 => {
            let b = match parts.next() {
                Some(id) => seq.participant(id, None)?,
                None => a,
            };
            NoteAnchor::Over(a.min(b), a.max(b))
        }
        1 => NoteAnchor::Left(a),
        _ => NoteAnchor::Right(a),
    };
    Some((text, anchor))
}

fn parse_seq_message(
    st: &str,
    seq: &mut Sequence,
) -> Option<(usize, usize, Option<String>, bool, SeqHead)> {
    let mut found: Option<(usize, &str, bool, SeqHead)> = None;
    for (pos, _) in st.char_indices() {
        for &(op, dashed, head) in SEQ_OPS {
            if st[pos..].starts_with(op) {
                found = Some((pos, op, dashed, head));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (pos, op, dashed, head) = found?;
    let from_id = st[..pos].trim();
    if from_id.is_empty() {
        return None;
    }
    let rest = st[pos + op.len()..]
        .trim_start()
        .trim_start_matches(['+', '-']);
    let (to_id, text) = match rest.split_once(':') {
        Some((to, text)) => (to.trim(), non_empty(decode_html_entities(text.trim()))),
        None => (rest.trim(), None),
    };
    if to_id.is_empty() {
        return None;
    }
    let from = seq.participant(from_id, None)?;
    let to = seq.participant(to_id, None)?;
    Some((from, to, text, dashed, head))
}

// ─── Layout ──────────────────────────────────────────────────

fn note_geometry(xs: &[usize], anchor: &NoteAnchor, text_w: usize) -> (usize, usize) {
    match *anchor {
        NoteAnchor::Over(l, r) => {
            let center = (xs[l] + xs[r]) / 2;
            let w = (xs[r] - xs[l] + 5).max(text_w + 2 * PAD + 2);
            (center.saturating_sub(w / 2), w)
        }
        NoteAnchor::Left(i) => {
            let w = text_w + 2 * PAD + 2;
            (xs[i].saturating_sub(2 + w - 1), w)
        }
        NoteAnchor::Right(i) => (xs[i] + 2, text_w + 2 * PAD + 2),
    }
}

fn draw_sequence_box(canvas: &mut Canvas, p: &Placed, label: &str, _shape: Shape) {
    // Simple rect box
    let box_chars = ['┌', '┐', '└', '┘', '─', '│'];
    let x = p.x;
    let y = p.y;
    let w = p.w;
    let h = p.h;

    // Top
    if y < canvas.h && x < canvas.w {
        canvas.cells[y * canvas.w + x] = box_chars[0]; // ┌
        canvas.occupied[y * canvas.w + x] = true;
    }
    for i in 1..w.saturating_sub(1) {
        if y < canvas.h && x + i < canvas.w {
            canvas.cells[y * canvas.w + x + i] = box_chars[4]; // ─
            canvas.occupied[y * canvas.w + x + i] = true;
        }
    }
    if y < canvas.h && x + w.saturating_sub(1) < canvas.w {
        canvas.cells[y * canvas.w + x + w.saturating_sub(1)] = box_chars[1]; // ┐
        canvas.occupied[y * canvas.w + x + w.saturating_sub(1)] = true;
    }

    // Sides + bottom
    for row in 1..h.saturating_sub(1) {
        if y + row < canvas.h {
            if x < canvas.w {
                canvas.cells[(y + row) * canvas.w + x] = box_chars[5]; // │
                canvas.occupied[(y + row) * canvas.w + x] = true;
            }
            if x + w.saturating_sub(1) < canvas.w {
                canvas.cells[(y + row) * canvas.w + x + w.saturating_sub(1)] = box_chars[5];
                canvas.occupied[(y + row) * canvas.w + x + w.saturating_sub(1)] = true;
            }
        }
    }

    // Bottom
    if y + h.saturating_sub(1) < canvas.h {
        if x < canvas.w {
            canvas.cells[(y + h.saturating_sub(1)) * canvas.w + x] = box_chars[2]; // └
            canvas.occupied[(y + h.saturating_sub(1)) * canvas.w + x] = true;
        }
        for i in 1..w.saturating_sub(1) {
            if x + i < canvas.w {
                canvas.cells[(y + h.saturating_sub(1)) * canvas.w + x + i] = box_chars[4];
                canvas.occupied[(y + h.saturating_sub(1)) * canvas.w + x + i] = true;
            }
        }
        if x + w.saturating_sub(1) < canvas.w {
            canvas.cells[(y + h.saturating_sub(1)) * canvas.w + x + w.saturating_sub(1)] = box_chars[3]; // ┘
            canvas.occupied[(y + h.saturating_sub(1)) * canvas.w + x + w.saturating_sub(1)] = true;
        }
    }

    // Label centered in the top box
    let label_w = label.chars().map(char_width).sum::<usize>();
    let label_x = x + (w.saturating_sub(label_w)) / 2;
    let label_y = y + h / 2;
    if label_y < canvas.h {
        for (i, c) in label.chars().enumerate() {
            if label_x + i < canvas.w {
                canvas.cells[label_y * canvas.w + label_x + i] = c;
                canvas.occupied[label_y * canvas.w + label_x + i] = true;
            }
        }
    }
}

pub fn layout_sequence(seq: &Sequence) -> String {
    let n = seq.labels.len();
    if n == 0 {
        return String::new();
    }

    let labels: Vec<String> = seq
        .labels
        .iter()
        .map(|l| fit_label(l, WRAP_WIDTH))
        .collect();
    let box_w: Vec<usize> = labels
        .iter()
        .map(|l| l.lines().map(|ln| ln.chars().count()).max().unwrap_or(1).max(1) + 2 * PAD + 2)
        .collect();
    let box_h: Vec<usize> = labels
        .iter()
        .map(|l| l.lines().count().max(1) + 2)
        .collect();
    let max_box_h = *box_h.iter().max().unwrap_or(&3);

    let item_text_w = |text: &Option<String>| text.as_deref().map(|t| t.chars().count()).unwrap_or(0);

    // Compute gaps between columns
    let mut gaps: Vec<usize> = (0..n.saturating_sub(1))
        .map(|i| SEQ_GAP.max(box_w[i].div_ceil(2) + box_w[i + 1].div_ceil(2) + 1))
        .collect();

    let mut reqs: Vec<(usize, usize, usize)> = Vec::new();
    for item in &seq.items {
        match item {
            SeqItem::Message { from, to, text, .. } => {
                let tw = item_text_w(text);
                if from != to {
                    let (l, r) = (*from.min(to), *from.max(to));
                    reqs.push((l, r, (tw + 2).max(4)));
                } else if *from + 1 < n {
                    reqs.push((*from, *from + 1, 5 + tw + 2));
                }
            }
            SeqItem::Note { anchor, text } => {
                let tw = text.chars().count();
                match *anchor {
                    NoteAnchor::Over(l, r) if l < r => reqs.push((l, r, tw.saturating_sub(1))),
                    NoteAnchor::Over(i, _) => {
                        let half = (tw + 4).div_ceil(2) + 2;
                        if i > 0 {
                            reqs.push((i - 1, i, half));
                        }
                        if i + 1 < n {
                            reqs.push((i, i + 1, half));
                        }
                    }
                    NoteAnchor::Left(i) if i > 0 => reqs.push((i - 1, i, tw + 7)),
                    NoteAnchor::Right(i) if i + 1 < n => reqs.push((i, i + 1, tw + 7)),
                    _ => {}
                }
            }
            SeqItem::Divider { .. } => {}
        }
    }
    reqs.sort_by_key(|&(l, r, _)| r - l);
    for (l, r, need) in reqs {
        let cur: usize = gaps[l..r].iter().sum();
        if cur < need {
            gaps[r - 1] += need - cur;
        }
    }

    let mut xs = vec![0usize; n];
    xs[0] = box_w[0] / 2;
    for i in 1..n {
        xs[i] = xs[i - 1] + gaps[i - 1];
    }

    let mut canvas_w = xs[n - 1] + box_w[n - 1].div_ceil(2) + 1;
    for item in &seq.items {
        match item {
            SeqItem::Message { from, to, text, .. } if from == to => {
                canvas_w = canvas_w.max(xs[*from] + 5 + item_text_w(text) + 1);
            }
            SeqItem::Note { anchor, text } => {
                let (x, w) = note_geometry(&xs, anchor, text.chars().count());
                canvas_w = canvas_w.max(x + w + 1);
            }
            SeqItem::Divider { text } => {
                canvas_w = canvas_w.max(text.chars().count() + 4);
            }
            _ => {}
        }
    }

    let mut rows: Vec<usize> = Vec::with_capacity(seq.items.len());
    let mut y = max_box_h + 1;
    for item in &seq.items {
        rows.push(y);
        y += match item {
            SeqItem::Message { from, to, text, .. } => {
                if from == to { 4 } else if text.is_some() { 3 } else { 2 }
            }
            SeqItem::Note { .. } => 4,
            SeqItem::Divider { .. } => 2,
        };
    }
    let bottom_top = y;
    let canvas_h = bottom_top + max_box_h;

    if canvas_w.saturating_mul(canvas_h) > MAX_CANVAS_CELLS {
        return fallback_seq(seq);
    }

    let mut canvas = Canvas::new(canvas_w, canvas_h);

    // Draw participant boxes at top and bottom
    for i in 0..n {
        for by in [0, bottom_top] {
            let p = Placed {
                x: xs[i].saturating_sub(box_w[i] / 2),
                y: by,
                w: box_w[i],
                h: box_h[i],
                cx: xs[i],
                cy: by + box_h[i] / 2,
                rank: 0,
            };
            draw_sequence_box(&mut canvas, &p, &labels[i], Shape::Rect);
        }
    }

    // Draw lifelines
    for i in 0..n {
        let lx = xs[i];
        for row in max_box_h..bottom_top {
            if row < canvas_h && lx < canvas.w {
                if canvas.cells[row * canvas.w + lx] == ' ' {
                    canvas.cells[row * canvas.w + lx] = '│';
                }
            }
        }
    }

    // Draw items
    for (item, &r) in seq.items.iter().zip(&rows) {
        match item {
            SeqItem::Message { from, to, text, dashed, head } => {
                let l = *from.min(to);
                let ri = *from.max(to);
                let x0 = xs[l].min(xs[ri]);
                let x1 = xs[l].max(xs[ri]);
                let is_left_to_right = from < to;

                // Horizontal line
                let line_char = if *dashed { '┄' } else { '─' };
                for x in x0..=x1 {
                    if r < canvas.h && x < canvas.w {
                        let current = canvas.cells[r * canvas.w + x];
                        if current == ' ' || current == '│' {
                            canvas.cells[r * canvas.w + x] = line_char;
                        } else if current == '─' || current == '┄' {
                            // crossing, use ┼
                            canvas.cells[r * canvas.w + x] = '┼';
                        }
                    }
                }

                // Arrow heads
                let arrow = match head {
                    SeqHead::Arrow => if is_left_to_right { '▶' } else { '◀' },
                    SeqHead::Cross => '╳',
                };
                if is_left_to_right {
                    if r < canvas.h && x1 < canvas.w {
                        canvas.cells[r * canvas.w + x1] = arrow;
                    }
                } else {
                    if r < canvas.h && x0 < canvas.w {
                        canvas.cells[r * canvas.w + x0] = arrow;
                    }
                }

                // Text label above the arrow
                if let Some(ref t) = text {
                    let mid = (x0 + x1) / 2;
                    let tx = mid.saturating_sub(t.chars().count() / 2);
                    if r > 0 {
                        for (j, c) in t.chars().enumerate() {
                            if tx + j < canvas.w {
                                canvas.cells[(r - 1) * canvas.w + tx + j] = c;
                            }
                        }
                    }
                }
            }
            SeqItem::Note { anchor, text } => {
                let tw = text.chars().count();
                let (nx, nw) = note_geometry(&xs, anchor, tw);
                let nh = 3;
                let ny = r;

                // Draw note box
                for row in 0..nh {
                    if ny + row >= canvas.h { continue; }
                    for col in 0..nw {
                        if nx + col >= canvas.w { continue; }
                        let ch = if row == 0 {
                            if col == 0 { '┌' }
                            else if col == nw - 1 { '┐' }
                            else { '─' }
                        } else if row == nh - 1 {
                            if col == 0 { '└' }
                            else if col == nw - 1 { '┘' }
                            else { '─' }
                        } else {
                            if col == 0 || col == nw - 1 { '│' }
                            else { ' ' }
                        };
                        canvas.cells[(ny + row) * canvas.w + nx + col] = ch;
                    }
                }
                // Text
                let tx = nx + 1;
                if ny + 1 < canvas.h {
                    for (j, c) in text.chars().enumerate() {
                        if tx + j < canvas.w {
                            canvas.cells[(ny + 1) * canvas.w + tx + j] = c;
                        }
                    }
                }
            }
            SeqItem::Divider { text } => {
                let dw = text.chars().count() + 4;
                let dx = 0usize;
                for col in 0..dw {
                    if r < canvas.h && dx + col < canvas.w {
                        canvas.cells[r * canvas.w + dx + col] = '─';
                    }
                }
                // Text on the divider line
                let tx = dx + 2;
                if r < canvas.h {
                    for (j, c) in text.chars().enumerate() {
                        if tx + j < canvas.w {
                            canvas.cells[r * canvas.w + tx + j] = c;
                        }
                    }
                }
            }
        }
    }

    canvas_to_string(&canvas)
}

fn fallback_seq(seq: &Sequence) -> String {
    let mut out = String::from("sequenceDiagram\n");
    for label in &seq.labels {
        out.push_str(&format!("  participant {}\n", label));
    }
    out
}

fn canvas_to_string(canvas: &Canvas) -> String {
    let mut out = String::new();
    let mut last_nonempty = 0;
    // Find last non-empty row
    for row in (0..canvas.h).rev() {
        let start = row * canvas.w;
        let end = start + canvas.w;
        if canvas.cells[start..end].iter().any(|&c| c != ' ') {
            last_nonempty = row;
            break;
        }
    }
    for row in 0..=last_nonempty {
        let start = row * canvas.w;
        let end = start + canvas.w;
        let line: String = canvas.cells[start..end].iter().collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let seq = parse_sequence("sequenceDiagram\n  Alice->>Bob: Hello").unwrap();
        assert_eq!(seq.labels, vec!["Alice", "Bob"]);
        assert_eq!(seq.items.len(), 1);
    }

    #[test]
    fn test_parse_participant() {
        let seq = parse_sequence(
            "sequenceDiagram\n  participant A as Alice\n  participant B as Bob\n  A->>B: Hi",
        )
        .unwrap();
        assert_eq!(seq.labels, vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_parse_autonumber() {
        let seq = parse_sequence(
            "sequenceDiagram\n  autonumber\n  A->>B: Hi\n  B->>A: Hey",
        )
        .unwrap();
        match &seq.items[0] {
            SeqItem::Message { text, .. } => assert!(text.as_ref().unwrap().starts_with("1.")),
            _ => panic!("expected message"),
        }
    }

    #[test]
    fn test_render_empty() {
        assert_eq!(layout_sequence(&Sequence { labels: vec![], index: HashMap::new(), items: vec![] }), "");
    }

    #[test]
    fn test_render_simple() {
        let seq = parse_sequence("sequenceDiagram\n  A->>B: hello").unwrap();
        let out = layout_sequence(&seq);
        assert!(!out.is_empty());
        assert!(out.contains("A"));
        assert!(out.contains("B"));
    }
}
