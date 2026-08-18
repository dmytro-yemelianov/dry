"""Test Python MachineCatalog and MachineProfile capabilities."""

from dry import Design, MachineCatalog, mm, mm_s


def test_machine_catalog_builtins():
    catalog = MachineCatalog()
    bambu = catalog.search(vendor="Bambu")
    assert len(bambu) >= 1
    x1c = next((m for m in bambu if m.id in ["bambu-x1c", "bambu-x1-carbon"]), None)
    assert x1c is not None
    assert x1c.bounds == (0.0, 256.0, 0.0, 256.0, 0.0, 256.0)

    cncs = catalog.search(category="cnc_mill")
    assert len(cncs) == 2  # Shapeoko 4 and Haas VF-2


def test_machine_profile_compatibility_integration():
    catalog = MachineCatalog()
    voron = catalog.get("voron-v24-350")
    assert voron.id == "voron-v24-350"

    # Design within Voron 350 volume
    safe_design = (
        Design()
        .point(mm(10), mm(10), mm(0))
        .speed(mm_s(200))
        .point(mm(300), mm(300), mm(50))
    )
    safe_report = safe_design.check_compatibility(voron.to_capabilities())
    assert safe_report["compatible"] is True
    assert len(safe_report["findings"]) == 0

    # Design exceeding Voron volume (X = 400mm)
    bad_design = (
        Design()
        .point(mm(10), mm(10), mm(0))
        .speed(mm_s(200))
        .point(mm(400), mm(300), mm(50))
    )
    bad_report = bad_design.check_compatibility(voron.to_capabilities())
    assert bad_report["compatible"] is False
    assert any(f["code"] == "OUT_OF_BOUNDS_X" for f in bad_report["findings"])
