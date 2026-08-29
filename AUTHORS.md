# Authors & Academic Research Attribution

## 1. Primary Authors & Maintainers

Dry is created and maintained by:
- **Dmytro Yemelianov** (<dmytro@yemelianov.dev>) — Creator, Lead Architect & Maintainer

---

## 2. Research & Mathematical Foundations

Dry's deterministic toolpath compilation and verification engine implements algorithms and formal models founded on established scientific literature:

### Triply Periodic Minimal Surfaces (TPMS)
- **Ken A. Brakke** — *Surface Evolver* mathematical formulation and nodal approximation equations for minimal surfaces (Gyroid, Schwarz P/D, Neovius, I-WP, Lidinoid, Fischer-Koch S/Y, F-RD, Split P).
- **Alan H. Schoen** — Infinite periodic minimal surface geometry and lattice representations.

### Jerk-Bounded Kinematics & Clothoid Cornering
- **Euler-Cornu Spirals (Clothoids)** — Linear curvature transitions evaluated via numerical Fresnel integral series for bounded lateral jerk in high-speed toolpath optimization.
- **7-Phase S-Curve Trajectory Planning** — S-curve acceleration profiles with continuous third derivative (jerk) constraints for machine axis dynamics.

### Multi-Axis Kinematic Transforms & Robotics
- **Jacques Denavit & Richard S. Hartenberg** — Standard Denavit-Hartenberg (DH) parameter conventions for serial kinematics chains.
- **Robotic Articulated Motion** — Inverse and forward kinematics models for 5-axis CNC rotary tables (AC/BC/AB) and 6-axis industrial articulated arms (KUKA KRL and ABB RAPID dialects).

---

## 3. Developer Certificate of Origin (DCO 1.1)

To protect the clean-room provenance and licensing integrity of Dry, all contributors certify the following Developer Certificate of Origin by including a `Signed-off-by: Name <email>` trailer in commit messages:

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source or source-available
    license indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    or source-available license and I have the right under that license
    to submit that work with modifications, whether created in whole
    or in part by me, under the same license; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source or source-available license(s)
    involved.
```
