# termermaid

Render Mermaid diagrams as Unicode box-drawing art in your terminal — pure Rust, zero dependencies on Node.js or browsers.

Ported from [xai-org/grok-build](https://github.com/xai-org/grok-build) (Apache 2.0).

## Supported diagram types

| Type | Support |
|------|---------|
| `flowchart` / `graph` | TD, LR, RL, BT; subgraphs; edge styles |
| `sequenceDiagram` | participants, messages, notes, autonumber, dividers |
| `stateDiagram-v2` | states, transitions, composite states |
| `classDiagram` | UML-style class boxes with members/methods, relations |
| `erDiagram` | entity boxes with attributes, cardinality relations |

## Install

```bash
cargo install termermaid
```

## Usage

```bash
# From stdin
echo "graph TD; A-->B; B-->C" | termermaid

# From file
termermaid diagram.mmd

# Flowchart
echo 'graph TD
  A[Start] --> B{Decision?}
  B -->|Yes| C[Do thing]
  B -->|No| D[Skip]' | termermaid

# Class diagram
echo 'classDiagram
  class Animal {
    +String name
    +int age
    +eat() void
  }
  Animal <|-- Dog' | termermaid

# Sequence diagram
echo 'sequenceDiagram
  Alice->>Bob: Hello
  Bob-->>Alice: Hi there!' | termermaid
```

Output examples:

```
╭       ╮              ╭               ╮
│ Start │──────▶│ Decision? │
╰       ╯              ╰       △       ╯
                               │
                    ┌──────────┼──────────┐
                    │          │          │
                    ▼          │          ▼
              ╭─────────╮     │    ╭──────╮
              │ Do thing │     │    │ Skip │
              ╰─────────╯     │    ╰──────╯
                               │
```

```
╭               ╮
     Animal
├               ┤
  +String name
    +int age
├               ┤
   +eat() void
╰       △       ╯
        │
╭───────╮
   Dog
╰───────╯
```

## Library

```rust
use termermaid::render;

let output = render("graph TD; A-->B").unwrap();
println!("{}", output);
```

## License

Apache 2.0 — extracted from xai-org/grok-build under the same license.
