//! Terminal Mermaid diagram renderer — CLI entry point.
//!
//! Ported from Grok Build `xai-grok-markdown` (Apache 2.0).

use termermaid::mermaid::render;

fn main() {
    let input = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
    if input.trim().is_empty() {
        eprintln!("Usage: echo 'graph TD; A-->B' | termermaid");
        std::process::exit(1);
    }
    match render(&input) {
        Some(art) => println!("{}", art),
        None => eprintln!("(unsupported or empty diagram)"),
    }
}
