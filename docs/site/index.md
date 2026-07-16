---
layout: home
hero:
  name: Dry
  text: Toolpath compiler — live docs
  tagline: Edit the code, watch the engine run. The same Rust/wasm engine the CLI and SDKs use.
  actions:
    - theme: brand
      text: Start the tour
      link: /guide/
    - theme: alt
      text: Browse 28 FullControl samples
      link: /gallery/?source=fullcontrol&design=nonplanar_spacer
---

<LiveExample src="author" :outputs="['gcode', 'ir']" />
