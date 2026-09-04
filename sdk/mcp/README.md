# `@dry/mcp` — DryMachina Model Context Protocol (MCP) Server

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](../../LICENSE)

Model Context Protocol (MCP) server that connects LLM coding assistants (Claude Desktop, Cursor, Goose, Copilot) directly to DryMachina's deterministic CAM compiler and verification engine.

---

## 1. Quickstart

### Installation & Build
```bash
cd sdk/mcp
npm install
npm run build
```

### Configuration (Claude Desktop / Cursor)
Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "dry": {
      "command": "node",
      "args": ["/path/to/dry/sdk/mcp/dist/index.js"]
    }
  }
}
```

---

## 2. Available MCP Tools

- `dry_simulate`: Simulates cycle time, material deposition, and kinematic motion metrics.
- `dry_verify_gcode`: Checks raw G-code against safety contracts (bounds, speed, collisions).
- `dry_generate_tpms`: Generates TPMS cellular infill lattices (Gyroid, Schwarz P/D).
- `dry_generate_pocket`: Generates high-speed CNC pocket milling toolpaths with helical ramps.
- `dry_generate_lathe`: Generates 2-axis CNC lathe facing and OD turning toolpaths.

---

## License

Licensed under the **Business Source License 1.1 (BUSL-1.1)**. The DryMachina v0.10.0
terms convert to MIT on 2030-09-05; see [LICENSE](../../LICENSE).
