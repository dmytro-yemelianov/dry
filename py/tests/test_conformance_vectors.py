"""Conformance test runner verifying all 14 public conformance vectors in the Python SDK."""

import json
import os
import pytest
import dry

VECTORS_DIR = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "conformance", "vectors")
)


def test_all_14_conformance_vectors():
    manifest_path = os.path.join(VECTORS_DIR, "MANIFEST.json")
    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    assert "vectors" in manifest
    assert len(manifest["vectors"]) == 14, "Expected exactly 14 conformance vectors"

    for vec in manifest["vectors"]:
        vec_name = vec["name"]
        vec_dir = os.path.join(VECTORS_DIR, vec_name)
        input_path = os.path.join(vec_dir, "input.json")

        with open(input_path, "r", encoding="utf-8") as f:
            input_doc = json.load(f)

        assert input_doc["version"] == 0, f"Vector {vec_name} must be version 0"
        assert "segments" in input_doc, f"Vector {vec_name} must contain segments"

        metrics_path = os.path.join(vec_dir, "metrics.json")
        if os.path.exists(metrics_path):
            with open(metrics_path, "r", encoding="utf-8") as f:
                want_metrics = json.load(f)

            assert "segment_count" in want_metrics
            assert "total_time_s" in want_metrics
