//! Terminal Mermaid diagram renderer — CLI entry point.
//!
//! Ported from Grok Build `xai-grok-markdown` (Apache 2.0).

use std::io::{self, Read};
use termermaid::mermaid::{render_with_opts, RenderOptions};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut ascii = false;
    let mut format_json = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ascii" | "-a" => ascii = true,
            "--json" | "-j" => format_json = true,
            "--help" | "-h" => {
                eprintln!("Usage: echo 'graph TD; A-->B' | termermaid [--ascii] [--json]");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap_or_default();
    if input.trim().is_empty() {
        eprintln!("Usage: echo 'graph TD; A-->B' | termermaid [--ascii] [--json]");
        std::process::exit(1);
    }

    let opts = RenderOptions {
        ascii_only: ascii,
        format_json,
    };

    match render_with_opts(&input, opts) {
        Some(art) => println!("{}", art),
        None => {
            if format_json {
                println!(r#"{{"error":"unsupported or empty diagram"}}"#);
            } else {
                eprintln!("(unsupported or empty diagram)");
            }
        }
    }
}
