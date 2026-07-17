use unicode_width::UnicodeWidthChar;

pub fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

pub const MAX_LABEL: usize = 28;
pub const PAD: usize = 1;
pub const GAP_X: usize = 3;
pub const GAP_Y: usize = 2;
pub const WRAP_WIDTH: usize = 24;
pub const MAX_LINES: usize = 4;
pub const CONT: char = '\u{0}';

pub fn wrap_label(label: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;

    for word in label.split_inclusive(|c: char| c.is_whitespace()) {
        let w: usize = word.chars().map(char_width).sum();
        if current.is_empty() {
            current.push_str(word);
            current_w = w;
        } else if current_w + w <= max_width {
            current.push_str(word);
            current_w += w;
        } else {
            if lines.len() + 1 >= max_lines {
                current.push('…');
                lines.push(current);
                return lines;
            }
            lines.push(current);
            current = word.to_string();
            current_w = w;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn fit_label(text: &str, max_width: usize) -> String {
    let tw: usize = text.chars().map(char_width).sum();
    if tw <= max_width {
        return text.to_string();
    }
    let mut out = String::with_capacity(max_width);
    let mut w = 0usize;
    for c in text.chars() {
        let cw = char_width(c).max(1);
        if w + cw > max_width {
            break;
        }
        out.push(c);
        w += cw;
    }
    out
}
