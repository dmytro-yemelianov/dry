# `dry-license` — Offline Cryptographic Licensing

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](../../LICENSE)

`dry-license` provides offline Ed25519 digital signature verification, grace period handling, and license stamping for DryMachina.

---

## 1. Design & Security

- **100% Offline**: No telemetry, no phone-home, no network dependency.
- **Ed25519 Asymmetric Signatures**: Cryptographically signed by the copyright holder's offline private key.
- **14-Day Grace Period**: Built-in grace period allows seamless operation during license renewals.
- **Report Stamping**: Stamped on verification and review reports.

---

## License

Licensed under the **Business Source License 1.1 (BUSL-1.1)**. The DryMachina v0.10.0
terms convert to MIT on 2030-09-05; see [LICENSE](../../LICENSE).
