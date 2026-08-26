# 4. Verify

`.verify({ bounds, maxFlow, ... })` checks the resolved toolpath against machine-safety contracts
and returns findings. The example prints a point outside the build volume; shrink or grow the
bounds and watch the out-of-bounds finding appear and clear.

Every contract is optional, and an omitted one leaves its rule disabled — a report tells you which
rules were actually in force, so a clean result is never mistaken for an unchecked one. In
TypeScript pass a `VerifyOptions` object; in Python use keyword arguments:

```ts
design.verify({ bounds: [[0, 250], [0, 210], [0, 220]], maxFlow: 15 });
```

```python
design.verify(bounds=[[0, 250], [0, 210], [0, 220]], max_flow=15)
```

The older TypeScript form that took twelve positional arguments still works and is deprecated:
reaching the tenth contract meant writing nine placeholders first, and miscounting them shifted
every later contract silently.

Reference: [verification types](../reference/generated/verification), [TypeScript `Report`](../reference/generated/typescript-sdk/types#report),
[Python `verify`](../reference/generated/python-sdk/design#verify).

<LiveExample src="verify" :outputs="['verify', 'gcode']" />
