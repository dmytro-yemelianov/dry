# `dry-cli` (`dry`) — Command-Line Compiler Interface

[![License: FSL-1.1-MIT](https://img.shields.io/badge/License-FSL--1.1--MIT-blue.svg)](../../LICENSE)

The standalone CLI binary for the Dry toolpath compiler.

---

## 1. Quickstart

### Build
```bash
cargo build -p dry-cli --release
```

### Common Commands
```bash
# 1. Emit G-code from Dry IR
dry emit model.dry.json -o output.gcode

# 2. Inspect & Simulate cycle metrics
dry inspect model.dry.json
dry simulate model.dry.json

# 3. Post-slicer G-code Review & Safety Verification
dry review-gcode slicer_output.gcode --bounds 0,250,0,210,0,220 --max-feedrate 18000

# 4. G-code Normalization & Optimization (S-Curves, Clothoids, Arc-Fitting)
dry rewrite-gcode raw.gcode --optimize -o optimized.gcode

# 5. Machine Profile Resolution
dry printer inspect bambu-x1c-pla
dry printer resolve bambu-x1c-pla --material dry:material/pla --nozzle 0.4 -o profile.json

# 6. Commercial License Activation (offline Ed25519 token)
dry license activate 'DRY-LICENSE-V1.eyJpZCI6Ii4uLiJ9...'
dry license status
```

---

## License

Licensed under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**. See [LICENSE](../../LICENSE).
