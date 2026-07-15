# 6. Generative

The TPMS and star-polygon lattice generators emit op lists in the engine and hand you a `Design`.
Here is a gyroid infill block; swap `surface` for `schwarz-p`, `iwp`, `neovius`, or `frd`.

Reference: [generators](../reference/generated/generators), [`TpmsOptions`](../reference/generated/typescript-sdk/generators#tpmsoptions),
[examples matrix](../reference/generated/examples).

<LiveExample src="generative" :outputs="['gcode', 'ir']" />
