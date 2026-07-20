//! Terminal Mermaid diagram renderer — CLI entry point.
//!
//! Ported from Grok Build `xai-grok-markdown` (Apache 2.0).

use std::io::{self, IsTerminal, Read};
use termermaid::mermaid::{render_with_opts, RenderOptions};
use termermaid::theme::{ColorMode, Theme};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut ascii = false;
    let mut format_json = false;
    let mut color_flag = String::from("auto");
    let mut theme_name = String::from("default");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ascii" | "-a" => ascii = true,
            "--json" | "-j" => format_json = true,
            "--color" => {
                i += 1;
                if i < args.len() {
                    color_flag = args[i].clone();
                }
            }
            "--theme" => {
                i += 1;
                if i < args.len() {
                    theme_name = args[i].clone();
                }
            }
            "--help" | "-h" => {
                eprintln!("Usage: echo 'graph TD; A-->B' | termermaid [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --ascii, -a          Use ASCII characters instead of Unicode");
                eprintln!("  --json, -j           Output JSON format");
                eprintln!("  --color <mode>       Color mode: auto, always, never (default: auto)");
                eprintln!("  --theme <name>       Color theme: default, terra, neon, mono, amber, phosphor");
                eprintln!("  --help, -h           Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap_or_default();
    if input.trim().is_empty() {
        eprintln!("Usage: echo 'graph TD; A-->B' | termermaid [OPTIONS]");
        std::process::exit(1);
    }

    let is_tty = std::io::stdout().is_terminal();
    let color_mode = ColorMode::from_flag(&color_flag, is_tty);
    let theme = Theme::get(theme_name.parse().unwrap_or_default());

    let opts = RenderOptions {
        ascii_only: ascii,
        format_json,
        color_mode: if format_json { ColorMode::None } else { color_mode },
        theme,
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
