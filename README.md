# termermaid

Terminal Mermaid diagram renderer. Extracted from Grok Build.

Renders Mermaid diagrams as Unicode box-drawing art directly in the terminal.

## Supported diagram types

- flowchart / graph (TD, LR, RL, BT)
- sequenceDiagram
- stateDiagram-v2
- classDiagram
- erDiagram

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

# Pipe to file
termermaid diagram.mmd > output.txt
```

## License

Apache 2.0 — extracted from xai-org/grok-build under the same license.
