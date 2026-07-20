//! Layout engine: topological sort, grid placement, Unicode box-drawing.
//!
//! Ported from Grok Build `xai-grok-markdown/src/mermaid.rs` (Apache 2.0).

use crate::canvas::{char_width, wrap_label, CONT, GAP_X, GAP_Y, MAX_LABEL, PAD, WRAP_WIDTH};
use crate::graph::{Dir, Edge, Graph, Head, Shape};
use crate::parse::ClassInfo;

const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 512;
const MAX_CANVAS_CELLS: usize = 1 << 21;
const MAX_LINES: usize = 4;

// ─── Canvas ──────────────────────────────────────────────────

const U: u8 = 1;
const D: u8 = 2;
const L: u8 = 4;
const R: u8 = 8;

// Cell classification for colorized output
const CLS_NONE: u8 = 0;
const CLS_BORDER: u8 = 1;
const CLS_TEXT: u8 = 2;
const CLS_EDGE: u8 = 3;
const CLS_EDGE_LABEL: u8 = 4;

#[derive(Clone, Copy, PartialEq)]
enum Cls {
    Border,
    Text,
    Edge,
    EdgeLabel,
}

impl Cls {
    fn to_u8(self) -> u8 {
        match self {
            Cls::Border => CLS_BORDER,
            Cls::Text => CLS_TEXT,
            Cls::Edge => CLS_EDGE,
            Cls::EdgeLabel => CLS_EDGE_LABEL,
        }
    }
}

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<char>,
    pub occupied: Vec<bool>,
    lines: Vec<u8>,
    cls: Vec<u8>,
}

impl Canvas {
    pub    fn new(w: usize, h: usize) -> Self {
        if w == 0 || h == 0 || w * h > MAX_CANVAS_CELLS {
            return Self {
                w: 1,
                h: 1,
                cells: vec![' '],
                occupied: vec![false],
                lines: vec![0],
                cls: vec![0],
            };
        }
        Self {
            w,
            h,
            cells: vec![' '; w * h],
            occupied: vec![false; w * h],
            lines: vec![0; w * h],
            cls: vec![0; w * h],
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    fn set(&mut self, x: usize, y: usize, c: char, cls: Cls) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = c;
            self.occupied[i] = true;
            self.cls[i] = cls.to_u8();
        }
    }

    fn add_bits(&mut self, x: usize, y: usize, bits: u8) {
        if x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.lines[i] |= bits;
            // Lines from add_bits default to Edge classification
            if self.cls[i] == CLS_NONE {
                self.cls[i] = CLS_EDGE;
            }
        }
    }

    fn seg_v(&mut self, x: usize, y1: usize, y2: usize) {
        let lo = y1.min(y2);
        let hi = y1.max(y2);
        for y in lo..=hi {
            self.add_bits(x, y, U | D);
        }
    }

    fn seg_h(&mut self, y: usize, x1: usize, x2: usize) {
        let lo = x1.min(x2);
        let hi = x1.max(x2);
        for x in lo..=hi {
            self.add_bits(x, y, L | R);
        }
    }

    fn junction(&mut self, x: usize, y: usize, bits: u8) {
        self.add_bits(x, y, bits);
    }

    fn flip_vertical(&mut self) {
        // Mirror y-axis: row i becomes row (h-1-i)
        let mut new_cells = vec![' '; self.w * self.h];
        let mut new_occ = vec![false; self.w * self.h];
        let mut new_lines = vec![0u8; self.w * self.h];
        let mut new_cls = vec![0u8; self.w * self.h];
        for y in 0..self.h {
            let ny = self.h - 1 - y;
            for x in 0..self.w {
                let i = self.idx(x, y);
                let ni = ny * self.w + x;
                new_cells[ni] = self.cells[i];
                new_occ[ni] = self.occupied[i];
                new_lines[ni] = self.lines[i];
                new_cls[ni] = self.cls[i];
            }
        }
        // Mirror the vertical bit: U<->D
        for i in 0..new_lines.len() {
            let b = new_lines[i];
            new_lines[i] = ((b & U) << 1) | ((b & D) >> 1) | (b & (L | R));
        }
        self.cells = new_cells;
        self.occupied = new_occ;
        self.lines = new_lines;
        self.cls = new_cls;
    }

    fn flip_horizontal(&mut self) {
        for y in 0..self.h {
            for x in 0..self.w / 2 {
                let l = self.idx(x, y);
                let r = self.idx(self.w - 1 - x, y);
                self.cells.swap(l, r);
                self.occupied.swap(l, r);
                self.lines.swap(l, r);
                self.cls.swap(l, r);
            }
        }
        // Mirror horizontal bit: L<->R
        for i in 0..self.lines.len() {
            let b = self.lines[i];
            self.lines[i] = ((b & L) << 3) | ((b & R) >> 3) | (b & (U | D));
        }
    }

    fn to_lines(&self, _border_c: char, _text_c: char, _edge_c: char,
                color_mode: crate::theme::ColorMode,
                theme: &crate::theme::Theme) -> Vec<String> {
        use crate::theme::ColorMode;
        let color_on = color_mode != ColorMode::None;
        let node_fg = theme.node_fg.as_ref().map(|c| c.fg(color_mode));
        let edge_fg = theme.edge.as_ref().map(|c| c.fg(color_mode));
        let edge_label_fg = theme.edge_label.as_ref().map(|c| c.fg(color_mode));
        let start_end_fg = theme.start_end.as_ref().map(|c| c.fg(color_mode));

        let mut out: Vec<String> = Vec::with_capacity(self.h);
        let mut prev_color: Option<u8> = None; // cls of previous cell
        for y in 0..self.h {
            let mut line = String::with_capacity(self.w * 2);
            let mut x = 0;
            while x < self.w {
                let i = self.idx(x, y);
                let c = self.cells[i];
                if c == CONT {
                    x += 1;
                    continue;
                }
                let bits = self.lines[i];
                // Prefer explicit cell content over line characters
                let ch = if self.occupied[i] && c != ' ' {
                    c
                } else if bits != 0 {
                    line_char(bits)
                } else {
                    ' '
                };

                if color_on && ch != ' ' {
                    let cur_cls = self.cls[i];
                    let color_escape = if cur_cls == CLS_TEXT || cur_cls == CLS_BORDER {
                        // Node text and borders
                        if let Some(ref escape) = node_fg {
                            Some(escape.as_str())
                        } else {
                            None
                        }
                    } else if cur_cls == CLS_EDGE_LABEL {
                        if let Some(ref escape) = edge_label_fg {
                            Some(escape.as_str())
                        } else if let Some(ref escape) = edge_fg {
                            Some(escape.as_str())
                        } else {
                            None
                        }
                    } else if cur_cls == CLS_EDGE {
                        if let Some(ref escape) = edge_fg {
                            Some(escape.as_str())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Emit ANSI escape if color changed
                    let needs_escape = match (prev_color, color_escape) {
                        (Some(prev), Some(cur)) => color_class_key(prev) != color_class_key(cur_cls),
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    };

                    if needs_escape {
                        if let Some(escape) = color_escape {
                            line.push_str(escape);
                        } else {
                            line.push_str(crate::theme::RESET);
                        }
                    }
                    prev_color = Some(cur_cls);
                }

                line.push(ch);
                x += 1;
            }
            let trimmed = line.trim_end().to_string();
            out.push(trimmed);
        }
        while out.last().map_or(false, |l| l.is_empty()) {
            out.pop();
        }
        out
    }
}

/// Map cls to a simplified color class key for run-length encoding.
fn color_class_key(cls: u8) -> u8 {
    match cls {
        CLS_TEXT | CLS_BORDER => 1,
        CLS_EDGE => 2,
        CLS_EDGE_LABEL => 3,
        _ => 0,
    }
}

fn line_char(bits: u8) -> char {
    match bits {
        1 => '│',           // U
        2 => '│',           // D
        4 => '─',           // L
        8 => '─',           // R
        3 => '│',           // U|D
        12 => '─',          // L|R
        9 => '└',           // U|R
        10 => '┌',          // D|R
        5 => '┘',           // U|L
        6 => '┐',           // D|L
        13 => '┴',          // U|L|R
        14 => '┬',          // D|L|R
        7 => '┤',           // U|D|L
        11 => '├',          // U|D|R
        15 => '┼',          // U|D|L|R
        _ => ' ',
    }
}

// ─── Placement ───────────────────────────────────────────────

#[derive(Clone)]
pub struct Placed {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub cx: usize,
    pub cy: usize,
    pub rank: usize,
}

struct NodeSizes {
    box_w: Vec<usize>,
    box_h: Vec<usize>,
    lay_w: Vec<usize>,
    lay_h: Vec<usize>,
    extra_h: Vec<usize>,
    self_label_w: Vec<usize>,
}

struct RoutePlan {
    edge_bus: Vec<usize>,
    bus_tracks: Vec<usize>,
}

// ─── Topological Sort ────────────────────────────────────────

fn compute_ranks(graph: &Graph) -> Vec<usize> {
    let n = graph.nodes.len();
    let mut indeg = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &graph.edges {
        if e.from < n && e.to < n && e.from != e.to {
            adj[e.from].push(e.to);
            indeg[e.to] += 1;
        }
    }
    let mut ranks = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut pos = 0;
    while pos < queue.len() {
        let u = queue[pos];
        pos += 1;
        for &v in &adj[u] {
            ranks[v] = ranks[v].max(ranks[u] + 1);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push(v);
            }
        }
    }
    ranks
}

fn order_ranks(by_rank: &mut [Vec<usize>], edges: &[Edge], ranks: &[usize]) {
    // Barycenter heuristic: minimize edge crossings within each rank
    let max_iter = 8;
    for _ in 0..max_iter {
        let mut changed = false;
        // Forward pass
        for r in 1..by_rank.len() {
            if by_rank[r].len() <= 1 {
                continue;
            }
            let mut bary: Vec<(usize, f64)> = Vec::with_capacity(by_rank[r].len());
            for &v in &by_rank[r] {
                let mut sum = 0.0;
                let mut count = 0;
                for e in edges {
                    if e.to == v && e.from != e.to && ranks[e.from] < r {
                        // Find position of e.from in previous rank
                        if let Some(pos) = by_rank[r - 1].iter().position(|&x| x == e.from) {
                            sum += pos as f64;
                            count += 1;
                        }
                    }
                }
                let avg = if count > 0 {
                    sum / count as f64
                } else {
                    by_rank[r].len() as f64 / 2.0
                };
                bary.push((v, avg));
            }
            bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let new_order: Vec<usize> = bary.iter().map(|&(v, _)| v).collect();
            if new_order != by_rank[r] {
                by_rank[r] = new_order;
                changed = true;
            }
        }
        // Backward pass
        for r in (0..by_rank.len() - 1).rev() {
            if by_rank[r].len() <= 1 {
                continue;
            }
            let mut bary: Vec<(usize, f64)> = Vec::with_capacity(by_rank[r].len());
            for &v in &by_rank[r] {
                let mut sum = 0.0;
                let mut count = 0;
                for e in edges {
                    if e.from == v && e.from != e.to && ranks[e.to] > r {
                        if let Some(pos) = by_rank[r + 1].iter().position(|&x| x == e.to) {
                            sum += pos as f64;
                            count += 1;
                        }
                    }
                }
                let avg = if count > 0 {
                    sum / count as f64
                } else {
                    by_rank[r].len() as f64 / 2.0
                };
                bary.push((v, avg));
            }
            bary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let new_order: Vec<usize> = bary.iter().map(|&(v, _)| v).collect();
            if new_order != by_rank[r] {
                by_rank[r] = new_order;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn assign_positions(
    by_rank: &[Vec<usize>],
    lay_w: &[usize],
    gap: usize,
    _edges: &[Edge],
    _ranks: &[usize],
) -> Vec<usize> {
    // Simple: left-align within each rank, then center the rank
    let mut centers = vec![0usize; lay_w.len()];
    for row in by_rank {
        let total_w: usize = row.iter().map(|&i| lay_w[i]).sum::<usize>()
            + (row.len().saturating_sub(1)) * gap;
        let mut x = 0usize;
        for &idx in row {
            centers[idx] = x + lay_w[idx] / 2;
            x += lay_w[idx] + gap;
        }
        // Shift right to match the widest rank (for left alignment)
        let _ = total_w;
    }
    centers
}

// ─── TD Placement ────────────────────────────────────────────

fn place_td(
    ranks: &[usize],
    max_rank: usize,
    by_rank: &[Vec<usize>],
    sizes: &NodeSizes,
    graph: &Graph,
    placed: &mut [Placed],
) -> RoutePlan {
    let centers = assign_positions(by_rank, &sizes.lay_w, GAP_X, &graph.edges, ranks);

    let mut edge_bus = vec![0usize; graph.edges.len()];
    let mut bus_tracks = vec![0usize; max_rank + 1];
    for r in 0..max_rank {
        let spans = bus_spans_td(graph, ranks, &centers, r);
        if spans.is_empty() {
            continue;
        }
        let (assigned, count) = assign_tracks(&spans, graph.edges.len());
        for (idx, slot) in assigned {
            edge_bus[idx] = slot;
        }
        bus_tracks[r] = count;
    }

    let rank_h: Vec<usize> = by_rank
        .iter()
        .map(|row| {
            row.iter()
                .map(|&i| sizes.box_h[i] + sizes.extra_h[i])
                .max()
                .unwrap_or(3)
        })
        .collect();
    let mut rank_y = vec![0usize; max_rank + 1];
    for r in 1..=max_rank {
        let gap = GAP_Y.max(bus_tracks[r - 1] + 1);
        rank_y[r] = rank_y[r - 1] + rank_h[r - 1] + gap;
    }

    for (r, row) in by_rank.iter().enumerate() {
        for &idx in row {
            let w = sizes.box_w[idx];
            let h = sizes.box_h[idx];
            let cx = centers[idx];
            let x = cx.saturating_sub(w / 2);
            let y = rank_y[r] + (rank_h[r] - h - sizes.extra_h[idx]) / 2;
            placed[idx] = Placed {
                x,
                y,
                w,
                h,
                cx,
                cy: y + h / 2,
                rank: r,
            };
        }
    }

    RoutePlan {
        edge_bus,
        bus_tracks,
    }
}

fn bus_spans_td(
    graph: &Graph,
    ranks: &[usize],
    centers: &[usize],
    rank: usize,
) -> Vec<(usize, usize, usize)> {
    let mut spans = Vec::new();
    for (ei, e) in graph.edges.iter().enumerate() {
        if e.from == e.to {
            continue;
        }
        let rf = ranks[e.from];
        let rt = ranks[e.to];
        if rf <= rank && rank < rt {
            let x1 = centers[e.from].min(centers[e.to]);
            let x2 = centers[e.from].max(centers[e.to]);
            spans.push((ei, x1, x2));
        }
    }
    spans
}

fn assign_tracks(spans: &[(usize, usize, usize)], n_edges: usize) -> (Vec<(usize, usize)>, usize) {
    let mut by_start: Vec<_> = spans.iter().collect();
    by_start.sort_by_key(|&&(_, s, _)| s);
    let mut tracks: Vec<Option<usize>> = vec![None; n_edges];
    let mut track_end: Vec<usize> = Vec::new();
    for &&(ei, s, e) in &by_start {
        let mut assigned = false;
        for t in 0..track_end.len() {
            if track_end[t] <= s {
                tracks[ei] = Some(t);
                track_end[t] = e;
                assigned = true;
                break;
            }
        }
        if !assigned {
            tracks[ei] = Some(track_end.len());
            track_end.push(e);
        }
    }
    let assigned: Vec<_> = tracks
        .iter()
        .enumerate()
        .filter_map(|(i, &t)| t.map(|t| (i, t)))
        .collect();
    (assigned, track_end.len())
}

// ─── Drawing ─────────────────────────────────────────────────

pub fn draw_box(canvas: &mut Canvas, p: &Placed, lines: &[String], shape: Shape, ascii_only: bool) {
    let (x, y, w, h) = (p.x, p.y, p.w, p.h);
    if w < 2 || h < 2 {
        return;
    }
    let right = x + w - 1;
    let bottom = y + h - 1;

    let (tl, tr, bl, br) = if ascii_only {
        ('+', '+', '+', '+')
    } else {
        match shape {
            Shape::Round | Shape::Diamond => ('╭', '╮', '╰', '╯'),
            Shape::Rect => ('┌', '┐', '└', '┘'),
        }
    };
    canvas.set(x, y, tl, Cls::Border);
    canvas.set(right, y, tr, Cls::Border);
    canvas.set(x, bottom, bl, Cls::Border);
    canvas.set(right, bottom, br, Cls::Border);

    for cx in (x + 1)..right {
        canvas.add_bits(cx, y, L | R);
        canvas.add_bits(cx, bottom, L | R);
    }
    for cy in (y + 1)..bottom {
        canvas.add_bits(x, cy, U | D);
        canvas.add_bits(right, cy, U | D);
    }

    for cy in y..=bottom {
        for cx in x..=right {
            let i = canvas.idx(cx, cy);
            canvas.occupied[i] = true;
            canvas.cls[i] = CLS_BORDER;
        }
    }

    let inner = w.saturating_sub(2 * PAD + 2).max(1);
    for (li, line) in lines.iter().enumerate() {
        let row = y + 1 + li;
        if row > bottom {
            break;
        }
        let text = crate::canvas::fit_label(line, inner);
        let tw: usize = text.chars().map(char_width).sum();
        let text_x = x + 1 + PAD + inner.saturating_sub(tw) / 2;
        let mut cur = text_x;
        for c in text.chars() {
            let cw = char_width(c).max(1);
            canvas.set(cur, row, c, Cls::Text);
            for k in 1..cw {
                canvas.set(cur + k, row, CONT, Cls::Text);
            }
            cur += cw;
        }
    }
}

fn route_forward(
    canvas: &mut Canvas,
    from: &Placed,
    to: &Placed,
    edge: &Edge,
    bus: usize,
) {
    let tx = to.cx;
    let bx = if from.cx.abs_diff(tx) <= 1 {
        tx
    } else {
        from.cx
    };
    let by = from.y + from.h - 1;
    let head_row = to.y.saturating_sub(1);

    canvas.junction(bx, by, D);
    canvas.seg_v(bx, by, bus);
    if bx == tx {
        canvas.seg_v(bx, bus, head_row);
    } else {
        canvas.seg_h(bus, bx, tx);
        canvas.seg_v(tx, bus, head_row);
    }

    if edge.head_to == Head::None {
        canvas.add_bits(tx, head_row, U);
    } else {
        canvas.set(
            tx,
            head_row,
            head_glyph(edge.head_to, '▼'),
            Cls::Edge,
        );
    }
    if edge.head_from != Head::None {
        canvas.set(bx, by, head_glyph(edge.head_from, '▲'), Cls::Edge);
    }

    if let Some(label) = &edge.label {
        place_label(canvas, label, head_row, tx + 1);
    }
}

fn head_glyph(head: Head, arrow: char) -> char {
    match head {
        Head::Circle => 'o',
        Head::Cross => '×',
        Head::DiamondFill => '◆',
        Head::DiamondOpen => '◇',
        Head::Triangle => match arrow {
            '▼' => '▽',
            '▲' => '△',
            _ => '▷',
        },
        Head::Arrow | Head::None => arrow,
    }
}

fn place_label(canvas: &mut Canvas, label: &str, row: usize, start_x: usize) {
    let label = if label.len() > MAX_LABEL {
        format!("{}…", &label[..MAX_LABEL - 1])
    } else {
        label.to_string()
    };
    let mut x = start_x;
    for c in label.chars() {
        if x + char_width(c).max(1) > canvas.w {
            break;
        }
        canvas.set(x, row, c, Cls::EdgeLabel);
        x += char_width(c).max(1);
    }
}

// ─── Public API ──────────────────────────────────────────────

/// Layout and render a flowchart/graph.
pub fn layout_flowchart(graph: &Graph, ascii_only: bool, color_mode: crate::theme::ColorMode, theme: &crate::theme::Theme) -> String {
    if graph.nodes.is_empty() {
        return String::new();
    }

    let ranks = compute_ranks(graph);
    let max_rank = *ranks.iter().max().unwrap_or(&0);

    let mut by_rank: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    for (idx, &r) in ranks.iter().enumerate() {
        by_rank[r].push(idx);
    }
    order_ranks(&mut by_rank, &graph.edges, &ranks);

    let n = graph.nodes.len();
    let wrapped: Vec<Vec<String>> = graph
        .nodes
        .iter()
        .map(|node| canvas_wrap(&node.label, WRAP_WIDTH, MAX_LINES))
        .collect();

    let box_w: Vec<usize> = wrapped
        .iter()
        .map(|lines| {
            lines.iter().map(|l| l.chars().map(char_width).sum::<usize>()).max().unwrap_or(1).max(1)
                + 2 * PAD + 2
        })
        .collect();
    let box_h: Vec<usize> = wrapped.iter().map(|lines| lines.len() + 2).collect();

    let mut extra_h = vec![0usize; n];
    for e in &graph.edges {
        if e.from == e.to && e.from < n {
            extra_h[e.from] = 2;
        }
    }
    let lay_w: Vec<usize> = (0..n).map(|i| box_w[i]).collect();
    let lay_h: Vec<usize> = (0..n).map(|i| box_h[i] + extra_h[i]).collect();

    let sizes = NodeSizes {
        box_w,
        box_h,
        lay_w,
        lay_h,
        extra_h,
        self_label_w: vec![0; n],
    };

    let mut placed: Vec<Placed> = vec![
        Placed {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            cx: 0,
            cy: 0,
            rank: 0,
        };
        n
    ];
    let route = place_td(&ranks, max_rank, &by_rank, &sizes, graph, &mut placed);

    let canvas_w = placed.iter().map(|p| p.x + p.w).max().unwrap_or(40) + 4;
    let canvas_h = placed.iter().map(|p| p.y + p.h).max().unwrap_or(20) + 4;
    let mut canvas = Canvas::new(canvas_w, canvas_h);

    // Draw edges first (behind boxes)
    for (ei, e) in graph.edges.iter().enumerate() {
        if e.from == e.to || e.from >= n || e.to >= n {
            continue;
        }
        let rf = ranks[e.from];
        let rt = ranks[e.to];
        if rf < rt {
            route_forward(&mut canvas, &placed[e.from], &placed[e.to], e, route.edge_bus[ei]);
        }
    }

    // Draw nodes
    for i in 0..n {
        draw_box(&mut canvas, &placed[i], &wrapped[i], graph.nodes[i].shape, ascii_only);
    }

    // Convert to string
    let lines = canvas.to_lines('│', ' ', '─', color_mode, theme);
    match graph.dir {
        Dir::Up => {
            // Reverse lines
            let mut rev: Vec<String> = lines.into_iter().rev().collect();
            // Flip vertical connectors
            for line in &mut rev {
                *line = line
                    .chars()
                    .map(|c| match c {
                        '┌' => '└',
                        '└' => '┌',
                        '┐' => '┘',
                        '┘' => '┐',
                        '╭' => '╰',
                        '╰' => '╭',
                        '╮' => '╯',
                        '╯' => '╮',
                        '┬' => '┴',
                        '┴' => '┬',
                        '▽' => '△',
                        '△' => '▽',
                        '▼' => '▲',
                        '▲' => '▼',
                        other => other,
                    })
                    .collect();
            }
            rev.join("\n")
        }
        Dir::Left => {
            // Rotate the canvas left
            if lines.is_empty() {
                return String::new();
            }
            let h = lines.len();
            let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);
            let mut rotated: Vec<String> = Vec::new();
            for x in (0..w).rev() {
                let mut row = String::new();
                for y in 0..h {
                    let ch = lines[y].chars().nth(x).unwrap_or(' ');
                    let mapped = match ch {
                        '─' => '│',
                        '│' => '─',
                        '┌' => '┐',
                        '┐' => '┘',
                        '└' => '┌',
                        '┘' => '└',
                        '╭' => '╮',
                        '╮' => '╯',
                        '╰' => '╭',
                        '╯' => '╰',
                        '├' => '┬',
                        '┤' => '┴',
                        '┬' => '┤',
                        '┴' => '├',
                        other => other,
                    };
                    row.push(mapped);
                }
                rotated.push(row.trim_end().to_string());
            }
            rotated.join("\n")
        }
        _ => lines.join("\n"),
    }
}

fn canvas_wrap(label: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;

    for c in label.chars() {
        if c == '\n' {
            let done = current;
            lines.push(done);
            if lines.len() >= max_lines {
                return lines;
            }
            current = String::new();
            current_w = 0;
            continue;
        }
        let cw = char_width(c).max(1);
        if current_w + cw > max_width && !current.is_empty() {
            let done = current;
            lines.push(done);
            if lines.len() >= max_lines {
                return lines;
            }
            current = String::new();
            current_w = 0;
        }
        current.push(c);
        current_w += cw;
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    // Truncate last line if needed
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            last.push('…');
        }
    }
    lines
}

// ─── Class/ER Diagram Layout ─────────────────────────────────

/// Render a class or ER diagram with member/attribute boxes.
pub fn layout_class_diagram(graph: &Graph, infos: &[ClassInfo], is_er: bool, ascii_only: bool, color_mode: crate::theme::ColorMode, theme: &crate::theme::Theme) -> String {
    let n = graph.nodes.len();
    if n == 0 {
        return String::new();
    }

    // Build sections per node: [name_section, members_section, methods_section]
    let mut node_sections: Vec<Vec<Vec<String>>> = Vec::with_capacity(n);
    let mut max_w = 1usize;

    for i in 0..n {
        let mut sections: Vec<Vec<String>> = Vec::new();

        // Name section
        let name = graph.nodes[i].label.clone();
        let name_section = if i < infos.len() {
            if let Some(ref stereo) = infos[i].stereotype {
                vec![format!("«{}»", stereo), name]
            } else {
                vec![name]
            }
        } else {
            vec![name]
        };
        sections.push(name_section);

        // Members/attributes section
        if i < infos.len() && !infos[i].members.is_empty() {
            let members: Vec<String> = infos[i].members.iter().map(|m| m.clone()).collect();
            sections.push(members);
        }

        // Methods section
        if i < infos.len() && !infos[i].methods.is_empty() {
            let methods: Vec<String> = infos[i].methods.iter().map(|m| m.clone()).collect();
            sections.push(methods);
        }

        // Compute width from longest line across all sections
        for sec in &sections {
            for line in sec {
                let lw = line.chars().map(char_width).sum::<usize>();
                max_w = max_w.max(lw);
            }
        }

        node_sections.push(sections);
    }

    // Wrap sections to fit width
    let inner = WRAP_WIDTH.saturating_sub(2 * PAD + 2).max(1);
    for sections in node_sections.iter_mut() {
        for sec in sections.iter_mut() {
            let mut wrapped: Vec<String> = Vec::new();
            for line in sec.drain(..) {
                let ws = wrap_label(&line, inner, MAX_LINES);
                wrapped.extend(ws);
            }
            *sec = wrapped;
        }
    }

    if graph.nodes.is_empty() {
        return String::new();
    }

    let ranks = compute_ranks(graph);
    let max_rank = *ranks.iter().max().unwrap_or(&0);

    let mut by_rank: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        by_rank[r].push(i);
    }
    for row in &mut by_rank {
        row.sort_by_key(|&i| &graph.nodes[i].label);
    }
    order_ranks(&mut by_rank, &graph.edges, &ranks);

    // Size boxes from actual sections, not just name labels
    let box_w: Vec<usize> = node_sections
        .iter()
        .map(|sections| {
            sections
                .iter()
                .flat_map(|s| s.iter())
                .map(|l| l.chars().map(char_width).sum::<usize>())
                .max()
                .unwrap_or(1)
                .max(1)
                + 2 * PAD
                + 2
        })
        .collect();
    let box_h: Vec<usize> = node_sections
        .iter()
        .map(|sections| {
            let lines: usize = sections.iter().map(|s| s.len()).sum();
            let seps = sections.len().saturating_sub(1);
            lines + seps + 2 // +2 for top/bottom border
        })
        .collect();

    let gap = GAP_X;
    let mut centers = vec![0usize; box_w.len()];
    let mut x = 0usize;
    let gap_y = GAP_Y;
    let mut y_per_rank = vec![0usize; max_rank + 1];
    let mut cur_y = 0usize;
    for rank in 0..=max_rank {
        y_per_rank[rank] = cur_y;
        let max_h = by_rank
            .get(rank)
            .map_or(3, |row| row.iter().map(|&i| box_h[i]).max().unwrap_or(3));
        cur_y += max_h + gap_y;
    }
    let canvas_h = cur_y + 1;

    // Place nodes per rank
    for (rank, row) in by_rank.iter().enumerate() {
        let mut pos_x = 0usize;
        for &idx in row {
            centers[idx] = pos_x + box_w[idx] / 2;
            pos_x += box_w[idx] + gap;
        }
        let total_w: usize = row.iter().map(|&i| box_w[i]).sum::<usize>()
            + (row.len().saturating_sub(1)) * gap;
        let offset = if total_w > 0 { 0usize } else { 0 };
        let mut cur = offset;
        for &idx in row {
            centers[idx] = cur + box_w[idx] / 2;
            cur += box_w[idx] + gap;
        }
    }
    let canvas_w = by_rank
        .iter()
        .flat_map(|row| row.iter())
        .map(|&i| centers[i] + box_w[i] / 2 + 1)
        .max()
        .unwrap_or(40)
        .max(40);

    // Build placed array
    let mut placed: Vec<Placed> = Vec::with_capacity(n);
    for i in 0..n {
        let rank = ranks[i];
        placed.push(Placed {
            x: centers[i].saturating_sub(box_w[i] / 2),
            y: y_per_rank[rank],
            w: box_w[i],
            h: box_h[i],
            cx: centers[i],
            cy: y_per_rank[rank] + box_h[i] / 2,
            rank,
        });
    }

    if canvas_w.saturating_mul(canvas_h) > MAX_CANVAS_CELLS {
        return layout_flowchart(graph, ascii_only, color_mode, theme);
    }

    let mut canvas = Canvas::new(canvas_w, canvas_h);

    // Draw class boxes
    for i in 0..n {
        let shape = &graph.nodes[i].shape;
        draw_class_box(&mut canvas, &placed[i], &node_sections[i], *shape, ascii_only);
    }

    // Edge routing — spans are (edge_index, from_rank, to_rank)
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    for (ei, e) in graph.edges.iter().enumerate() {
        if e.from < n && e.to < n && ranks[e.from] < ranks[e.to] {
            spans.push((ei, ranks[e.from], ranks[e.to]));
        }
    }
    let (assigned, _max_bus) = assign_tracks(&spans, graph.edges.len());
    let mut edge_bus = vec![0usize; graph.edges.len()];
    for (idx, slot) in assigned {
        edge_bus[idx] = slot;
    }

    for (ei, e) in graph.edges.iter().enumerate() {
        if e.from < n && e.to < n {
            let rf = ranks[e.from];
            let rt = ranks[e.to];
            if rf < rt {
                route_forward(&mut canvas, &placed[e.from], &placed[e.to], e, edge_bus[ei]);
            }
        }
    }

    canvas_to_string(&canvas, color_mode, theme)
}

fn canvas_to_string(canvas: &Canvas, color_mode: crate::theme::ColorMode, theme: &crate::theme::Theme) -> String {
    let color_on = color_mode != crate::theme::ColorMode::None;
    let node_fg = theme.node_fg.as_ref().map(|c| c.fg(color_mode)).unwrap_or_default();
    let edge_fg = theme.edge.as_ref().map(|c| c.fg(color_mode)).unwrap_or_default();
    let edge_label_fg = theme.edge_label.as_ref().map(|c| c.fg(color_mode)).unwrap_or_default();
    let reset = crate::theme::RESET;

    let mut out = String::new();
    let mut last_nonempty = 0;
    for row in (0..canvas.h).rev() {
        let start = row * canvas.w;
        let end = start + canvas.w;
        if canvas.cells[start..end].iter().any(|&c| c != ' ') {
            last_nonempty = row;
            break;
        }
    }
    let mut prev_cls: u8 = CLS_NONE;
    for row in 0..=last_nonempty {
        let start = row * canvas.w;
        let end = start + canvas.w;

        if color_on {
            let mut line = String::with_capacity(canvas.w * 2);
            for i in start..end {
                let ch = canvas.cells[i];
                let cur_cls = canvas.cls[i];
                if ch != ' ' && cur_cls != prev_cls {
                    match cur_cls {
                        CLS_TEXT | CLS_BORDER => {
                            if !node_fg.is_empty() { line.push_str(&node_fg); }
                        }
                        CLS_EDGE => {
                            if !edge_fg.is_empty() { line.push_str(&edge_fg); }
                        }
                        CLS_EDGE_LABEL => {
                            if !edge_label_fg.is_empty() { line.push_str(&edge_label_fg); }
                        }
                        _ => {
                            if prev_cls != CLS_NONE { line.push_str(reset); }
                        }
                    }
                    prev_cls = cur_cls;
                }
                line.push(ch);
            }
            let trimmed = line.trim_end().to_string();
            if !trimmed.is_empty() || row < last_nonempty {
                out.push_str(&trimmed);
                out.push('\n');
            }
        } else {
            let line: String = canvas.cells[start..end].iter().collect();
            let trimmed = line.trim_end().to_string();
            if !trimmed.is_empty() || row < last_nonempty {
                out.push_str(&trimmed);
                out.push('\n');
            }
        }
    }
    out
}

/// Draw a UML-style class box
pub fn draw_class_box(canvas: &mut Canvas, p: &Placed, sections: &[Vec<String>], shape: Shape, ascii_only: bool) {
    let (x, y, w, h) = (p.x, p.y, p.w, p.h);
    if w < 2 || h < 2 {
        return;
    }
    let right = x + w - 1;
    let bottom = y + h - 1;

    // Border corners
    let (tl, tr, bl, br, _hz, _vt) = if ascii_only {
        ('+', '+', '+', '+', '-', '|')
    } else {
        match shape {
            Shape::Round | Shape::Diamond => ('╭', '╮', '╰', '╯', '─', '│'),
            Shape::Rect => ('┌', '┐', '└', '┘', '─', '│'),
        }
    };
    canvas.set(x, y, tl, Cls::Border);
    canvas.set(right, y, tr, Cls::Border);
    canvas.set(x, bottom, bl, Cls::Border);
    canvas.set(right, bottom, br, Cls::Border);

    // Top and bottom edges
    for cx in (x + 1)..right {
        canvas.add_bits(cx, y, L | R);
        canvas.add_bits(cx, bottom, L | R);
    }

    // Row tracker
    let mut row = y + 1;

    for (si, section) in sections.iter().enumerate() {
        if section.is_empty() {
            continue;
        }
        // Section separator (except before first section)
        if si > 0 {
            if row <= bottom {
                canvas.set(x, row, if ascii_only { '+' } else { '├' }, Cls::Border);
                for cx in (x + 1)..right {
                    canvas.add_bits(cx, row, L | R);
                }
                canvas.set(right, row, if ascii_only { '+' } else { '┤' }, Cls::Border);
            }
            row += 1;
        }
        // Content lines
        let inner = w.saturating_sub(2 * PAD + 2).max(1);
        for line in section {
            if row > bottom {
                break;
            }
            let text = crate::canvas::fit_label(line, inner);
            let tw: usize = text.chars().map(char_width).sum();
            let text_x = x + 1 + PAD + inner.saturating_sub(tw) / 2;
            let mut cur = text_x;
            for c in text.chars() {
                let cw = char_width(c).max(1);
                canvas.set(cur, row, c, Cls::Text);
                for k in 1..cw {
                    canvas.set(cur + k, row, CONT, Cls::Text);
                }
                cur += cw;
            }
            row += 1;
        }
    }

    // Side edges for all rows
    for cy in (y + 1)..bottom {
        canvas.add_bits(x, cy, U | D);
        canvas.add_bits(right, cy, U | D);
    }

    // Mark all cells as occupied with Border classification
    for cy in y..=bottom {
        for cx in x..=right {
            let i = canvas.idx(cx, cy);
            canvas.occupied[i] = true;
            canvas.cls[i] = CLS_BORDER;
        }
    }
}
