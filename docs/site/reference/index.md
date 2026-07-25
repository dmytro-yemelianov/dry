# Reference

This section is generated from the Dry codebase and structured metadata.

Use the guide when learning the workflow. Use these pages when you need exact SDK methods, types, CLI behavior, IR concepts, or example coverage.

## Generated pages

- TypeScript SDK
  - [Overview](./generated/typescript-sdk.md)
  - [Design](./generated/typescript-sdk/design.md)
  - [Core types](./generated/typescript-sdk/types.md)
  - [Generator exports](./generated/typescript-sdk/generators.md)
- Python SDK
  - [Overview](./generated/python-sdk.md)
  - [Design](./generated/python-sdk/design.md)
  - [Module API](./generated/python-sdk/module.md)
- CLI
  - [Overview](./generated/cli.md)
  - Workflow
    - [inspect](./generated/cli/inspect.md)
    - [simulate](./generated/cli/simulate.md)
    - [emit](./generated/cli/emit.md)
    - [verify](./generated/cli/verify.md)
    - [optimize](./generated/cli/optimize.md)
    - [rewrite-gcode](./generated/cli/rewrite-gcode.md)
  - Inputs & assets
    - [import-gcode](./generated/cli/import-gcode.md)
    - [import-printer-cfg](./generated/cli/import-printer-cfg.md)
    - [pack](./generated/cli/pack.md)
    - [unpack](./generated/cli/unpack.md)
  - Analysis
    - [review-gcode](./generated/cli/review-gcode.md)
    - [trace-gcode](./generated/cli/trace-gcode.md)
    - [forensics-gcode](./generated/cli/forensics-gcode.md)
    - [explain](./generated/cli/explain.md)
    - [compare](./generated/cli/compare.md)
  - Operations
    - [upload](./generated/cli/upload.md)
- [IR](./generated/ir.md)
  - [Data model](./generated/ir/data-model.md)
  - [JSON wire form](./generated/ir/json-wire-form.md)
- [Generators](./generated/generators.md)
- [Verification](./generated/verification.md)
- [Profiles and reports](./generated/profiles-and-reports.md)
  - [Profile schema](./generated/profiles-and-reports/profile-schema.md)
  - [Rule catalog](./generated/profiles-and-reports/verification-rules.md)
  - [Report outputs](./generated/profiles-and-reports/report-outputs.md)
  - [Supported profile matrix](./generated/profiles-and-reports/supported-profile-matrix.md)
- [Examples](./generated/examples.md)
- [FullControl source audit](./fullcontrol-sources.md)

## Regeneration

Run from `docs/site`:

```sh
npm run reference
```

Generated files live under `docs/site/reference/generated/`.
