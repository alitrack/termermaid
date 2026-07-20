# termermaid

Render Mermaid diagrams as Unicode box-drawing art in your terminal — pure Rust, zero dependencies on Node.js or browsers.

Ported from [xai-org/grok-build](https://github.com/xai-org/grok-build) (Apache 2.0).

## Supported diagram types

| Type | Features |
|------|----------|
| `flowchart` / `graph` | TD, LR, RL, BT; subgraphs; edge styles (`-->`, `---`, `-.->`, `==>`) |
| `sequenceDiagram` | participants, messages, notes, autonumber, **loop/alt/opt blocks** |
| `stateDiagram-v2` | states, transitions, composite states |
| `classDiagram` | UML-style boxes (name / members / methods sections), relations |
| `erDiagram` | entity boxes with attributes, cardinality relations |
| `pie` | circular filled pie with sector legend |

## Install

```bash
cargo install termermaid
```

Python (CI builds coming):

```bash
pip install termermaid
```

## Usage

```bash
# From stdin
echo 'graph TD; A-->B; B-->C' | termermaid

# Options
echo 'graph TD; A-->B' | termermaid --ascii          # ASCII mode
echo 'graph TD; A-->B' | termermaid --json            # JSON output
echo 'graph TD; A-->B' | termermaid --color always --theme neon   # Colored
```

### Options

| Flag | Description |
|------|-------------|
| `--ascii`, `-a` | Use `+`, `-`, `\|` instead of Unicode box-drawing |
| `--json`, `-j` | Output JSON with width/height/text metadata |
| `--color <mode>` | `auto` (default, respects `NO_COLOR`), `always`, `never` |
| `--theme <name>` | `default`, `terra`, `neon`, `mono`, `amber`, `phosphor` |

### Color themes

```
echo 'graph TD; A-->B' | termermaid --color always --theme neon
echo 'graph TD; A-->B' | termermaid --color always --theme phosphor
```

6 themes: **default** (terminal colors) | **terra** (earthy) | **neon** (bright) | **mono** (high-contrast) | **amber** (classic CRT) | **phosphor** (green CRT)

### Examples

**Flowchart**
```bash
echo 'graph TD
  A[Start] --> B{Decision?}
  B -->|Yes| C[Do thing]
  B -->|No| D[Skip]
  C --> E((End))
  D --> E' | termermaid
```

**Sequence with blocks**
```bash
echo 'sequenceDiagram
  Alice->>Bob: Hello
  loop Every minute
    Alice->>Bob: Ping
    Bob-->>Alice: Pong
  end
  alt Success
    Bob->>Alice: OK
  else Failure
    Bob->>Alice: Error
  end' | termermaid
```

Output:
```
┌───────┐  ┌─────┐
│ Alice │  │ Bob │
└───────┘  └─────┘
    │  Hello  │
    ──────────▶
    │         │
│─loop Every minute─│
│   │  Ping   │   │
│   ──────────▶   │
│   │  Pong   │   │
│   ◀┄┄┄┄┄┄┄┄┄┄   │
│──────────────────│
│─alt Success──────│
│   │   OK    │   │
│   ◀──────────   │
│─else Failure─────│
│   │  Error  │   │
│   ◀──────────   │
│──────────────────│
    │         │
┌───────┐  ┌─────┐
│ Alice │  │ Bob │
└───────┘  └─────┘
```

**Class diagram**
```bash
echo 'classDiagram
  class Animal {
    +String name
    +int age
    +eat() void
  }
  Animal <|-- Dog' | termermaid
```

**Pie chart**
```bash
echo 'pie title Pets
  "Dogs" : 386
  "Cats" : 85
  "Rats" : 15' | termermaid
```

Output:
```
Pets
════
          ·          
    ··▓▓▓▒█████··    
  ··▓▓▓▓▓▒███████··  
 ·▓▓▓▓▓▓▓▓█████████· 
 ·█▓▓▓▓▓▓▓█████████· 
·███████████████████·
 ·█████████████████· 
 ·█████████████████· 
  ··█████████████··  
    ··█████████··    
          ·          
  █ Dogs — 79.4%
  ▓ Cats — 17.5%
  ▒ Rats — 3.1%
```

## Library

```rust
use termermaid::mermaid::{render_with_opts, RenderOptions};
use termermaid::theme::{ColorMode, Theme, ThemeType};

let opts = RenderOptions {
    ascii_only: false,
    format_json: false,
    color_mode: ColorMode::Ansi256,
    theme: Theme::get(ThemeType::Neon),
};
let output = render_with_opts("graph TD; A-->B", opts).unwrap();
println!("{}", output);
```

## Python

```python
import termermaid
print(termermaid.render('graph TD; A-->B'))
```

Build wheel: `maturin build --release --features python`

## Changelog

- **v0.5.0** — Sequence block rendering (loop/alt/opt/par boxes with vertical borders)
- **v0.4.0** — ANSI color themes (6 themes, `--color` / `--theme` flags)
- **v0.3.0** — Pie chart support
- **v0.2.0** — ASCII mode (`--ascii`), JSON output (`--json`), `RenderOptions` API
- **v0.1.0** — Initial release: flowchart, sequence, state, class, ER diagrams

## License

Apache 2.0 — extracted from xai-org/grok-build under the same license.
