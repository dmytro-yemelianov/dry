# Chapter 5: Executable End-to-End Examples

All examples in this chapter are located in `examples/python/` and `examples/typescript/` and can be executed and verified with a single command:

```bash
python3 examples/run_all.py
```

---

## 1. Example 01: Continuous-Z Parametric Spiral Vase

* **File**: `examples/python/01_spiral_vase.py`
* **Focus**: Single-stroke continuous Z printing without layer seams.
* **Verification**: Evaluates monotonic-Z and build volume constraints before emission.

```bash
python3 examples/python/01_spiral_vase.py
```

---

## 2. Example 02: TPMS Cellular Infill

* **File**: `examples/python/02_tpms_gyroid.py`
* **Focus**: High-performance mathematical implicit surface slicing.
* **Performance**: Generates $>25,000$ toolpath segments in $<200\text{ ms}$.

```bash
python3 examples/python/02_tpms_gyroid.py
```

---

## 3. Example 03: 5-Axis Surface Draping

* **File**: `examples/python/03_five_axis_drape.py`
* **Focus**: Toolframe orientation vector field $(i, j, k)$ and multi-axis kinematic lowering.
* **Output**: Emits rotary $A$ and $B$ words dynamically synchronized with linear moves.

```bash
python3 examples/python/03_five_axis_drape.py
```

---

## 4. Example 04: Subtractive CNC Pocket Milling

* **File**: `examples/python/04_cnc_pocket_milling.py`
* **Focus**: Multi-pass rectangular and circular pocketing with stepovers and clearance planes.
* **Output**: Industrial G-code compliant with CNC controllers.

```bash
python3 examples/python/04_cnc_pocket_milling.py
```

---

## 5. Example 05: Machine Catalog Pre-Flight Audit

* **File**: `examples/python/05_machine_catalog_preflight.py`
* **Focus**: Pre-flight validation against physical machine limits (Bambu, Prusa, Haas, Voron).
* **Guarantees**: Detects and refuses axis overtravel, excessive feedrates, and spindle speed violations before sending code to physical hardware.

```bash
python3 examples/python/05_machine_catalog_preflight.py
```
