use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    Rect,
    Round,
    Diamond,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Head {
    None,
    Arrow,
    Circle,
    Cross,
    Triangle,
    DiamondFill,
    DiamondOpen,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LineKind {
    Solid,
    Dotted,
    Thick,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Dir {
    Down,
    Up,
    Right,
    Left,
}

pub struct Node {
    pub label: String,
    pub shape: Shape,
}

pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    pub head_to: Head,
    pub head_from: Head,
    pub line: LineKind,
}

pub struct Group {
    pub id: String,
    pub label: String,
    pub parent: Option<usize>,
}

pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub index: HashMap<String, usize>,
    pub groups: Vec<Group>,
    pub node_group: Vec<Option<usize>>,
    pub cur_group: Option<usize>,
    pub over_cap: bool,
    pub dir: Dir,
}

impl Graph {
    pub fn node_index(&mut self, id: &str, label: Option<&str>, shape: Shape) -> Option<usize> {
        if let Some(&i) = self.index.get(id) {
            if let Some(label) = label {
                self.nodes[i].label = label.to_string();
                self.nodes[i].shape = shape;
            }
            return Some(i);
        }
        if self.nodes.len() >= 128 {
            self.over_cap = true;
            return None;
        }
        let label = label.unwrap_or(id).to_string();
        self.index.insert(id.to_string(), self.nodes.len());
        self.nodes.push(Node { label, shape });
        self.node_group.push(self.cur_group);
        Some(self.nodes.len() - 1)
    }

    pub fn node_label(&mut self, id: &str, label: &str) -> Option<usize> {
        if let Some(&i) = self.index.get(id) {
            self.nodes[i].label = label.to_string();
            return Some(i);
        }
        self.node_index(id, Some(label), Shape::Round)
    }
}
