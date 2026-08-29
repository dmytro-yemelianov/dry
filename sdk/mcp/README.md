# `@dry/mcp` — Dry Machina Model Context Protocol (MCP) Server

[![License: FSL-1.1-MIT](https://img.shields.io/badge/License-FSL--1.1--MIT-blue.svg)](../../LICENSE)

Model Context Protocol (MCP) server that connects LLM coding assistants (Claude Desktop, Cursor, Goose, Copilot) directly to Dry's deterministic CAM compiler and verification engine.

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

Licensed under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**. See [LICENSE](../../LICENSE).
