#!/usr/bin/env python3
"""
OpenToolpath (.otp) Container Validator
Validates .otp package archives against the normative OpenToolpath v1.0 standard
(docs/29-opentoolpath-spec.md) and JSON schema (spec/opentoolpath-v1.schema.json).
"""

import sys
import os
import json
import zipfile
import hashlib
import jsonschema

EXPECTED_MIMETYPE = b"application/vnd.opentoolpath+zip"
SCHEMA_PATH = os.path.join(os.path.dirname(__file__), "..", "spec", "opentoolpath-v1.schema.json")

def load_schemas():
    with open(SCHEMA_PATH, "r", encoding="utf-8") as f:
        root_schema = json.load(f)
    
    defs = root_schema.get("$defs", {})
    validators = {}
    for name in ["Manifest", "Toolpath", "ToolsCatalog", "ContextDescriptor"]:
        sub_schema = {
            "$schema": root_schema.get("$schema", "https://json-schema.org/draft/2020-12/schema"),
            "$defs": defs,
            **defs[name]
        }
        validators[name] = jsonschema.Draft202012Validator(sub_schema)
    return validators

def validate_otp_archive(otp_path, validators):
    print(f"Validating {otp_path}...")
    
    if not os.path.isfile(otp_path):
        raise ValueError(f"File not found: {otp_path}")

    # 1. Raw file inspection: check mimetype header at offset 30
    with open(otp_path, "rb") as f:
        raw = f.read(100)
    
    if len(raw) < 70 or raw[:4] != b"PK\x03\x04":
        raise ValueError("Not a valid PKZIP archive")
    
    # Check that mimetype is uncompressed store entry at start
    # Local header is 30 bytes, filename "mimetype" is 8 bytes => content at 38
    if raw[30:38] != b"mimetype":
        raise ValueError(f"First entry in ZIP must be mimetype, got {raw[30:38]!r}")
    if raw[38:38 + len(EXPECTED_MIMETYPE)] != EXPECTED_MIMETYPE:
        raise ValueError(f"Mimetype content mismatch: got {raw[38:38 + len(EXPECTED_MIMETYPE)]!r}")

    # 2. Inspect ZIP structure
    with zipfile.ZipFile(otp_path, "r") as zf:
        infolist = zf.infolist()
        namelist = zf.namelist()

        # Check zip slip and path sanitization (POSIX and Windows drive paths)
        for info in infolist:
            fn = info.filename
            if (
                fn.startswith("/")
                or "\\" in fn
                or ".." in fn.split("/")
                or (len(fn) >= 2 and fn[1] == ":")
            ):
                raise ValueError(f"Unsafe path in archive (zip slip / path traversal violation): {fn}")

        # Check mimetype is uncompressed (method 0)
        mimetype_info = zf.getinfo("mimetype")
        if mimetype_info.compress_type != zipfile.ZIP_STORED:
            raise ValueError(f"mimetype entry must be uncompressed (STORED), got method {mimetype_info.compress_type}")

        # Check required files
        for req in ["manifest.json", "tools.json", "context.json"]:
            if req not in namelist:
                raise ValueError(f"Missing required entry: {req}")

        # 3. Read and validate manifest.json
        manifest_bytes = zf.read("manifest.json")
        try:
            manifest_json = json.loads(manifest_bytes.decode("utf-8"))
        except Exception as e:
            raise ValueError(f"manifest.json is not valid JSON: {e}")

        errors = list(validators["Manifest"].iter_errors(manifest_json))
        if errors:
            raise ValueError(f"manifest.json schema violation: {errors[0].message} at {list(errors[0].path)}")

        # Validate package_id format if URN UUID
        pkg_id = manifest_json.get("package_id", "")
        if pkg_id.startswith("urn:uuid:"):
            import uuid
            try:
                uuid.UUID(pkg_id[9:])
            except Exception as e:
                raise ValueError(f"Invalid RFC 4122 UUID in package_id: {pkg_id} ({e})")

        # 4. Check digests of entries
        digests = manifest_json.get("digests", {})
        entrypoints = manifest_json.get("entrypoints", {})
        payload_entry = entrypoints.get("payload")
        if not payload_entry or payload_entry not in namelist:
            raise ValueError(f"Payload entrypoint {payload_entry} not found in archive")

        # All non-envelope, non-signature archive entries must be declared in digests
        # (docs/29-opentoolpath-spec.md §3.3)
        expected_envelope_files = {"mimetype", "manifest.json"}
        for name in namelist:
            if name in expected_envelope_files or name.startswith("signatures/"):
                continue
            if name not in digests:
                raise ValueError(f"Unlisted archive entry not declared in manifest.json digests: {name}")

        for internal_path, expected_digest in digests.items():
            if internal_path not in namelist:
                raise ValueError(f"Digested file {internal_path} missing from package archive")
            file_bytes = zf.read(internal_path)
            actual_digest = "sha256:" + hashlib.sha256(file_bytes).hexdigest()
            if actual_digest != expected_digest:
                raise ValueError(f"Cryptographic digest mismatch for {internal_path}: expected {expected_digest}, actual {actual_digest}")

        # 5. Validate payload
        payload_format = entrypoints.get("payload_format", "json")
        if payload_format == "json":
            payload_bytes = zf.read(payload_entry)
            payload_json = json.loads(payload_bytes.decode("utf-8"))
            errors = list(validators["Toolpath"].iter_errors(payload_json))
            if errors:
                raise ValueError(f"Payload {payload_entry} schema violation: {errors[0].message} at {list(errors[0].path)}")

        # 6. Validate tools.json
        tools_bytes = zf.read("tools.json")
        tools_json = json.loads(tools_bytes.decode("utf-8"))
        errors = list(validators["ToolsCatalog"].iter_errors(tools_json))
        if errors:
            raise ValueError(f"tools.json schema violation: {errors[0].message} at {list(errors[0].path)}")

        # 7. Validate context.json
        context_bytes = zf.read("context.json")
        context_json = json.loads(context_bytes.decode("utf-8"))
        errors = list(validators["ContextDescriptor"].iter_errors(context_json))
        if errors:
            raise ValueError(f"context.json schema violation: {errors[0].message} at {list(errors[0].path)}")

    print(f"PASS: {otp_path} is 100% conforming OpenToolpath v1.0 package.")
    return True

def main():
    if len(sys.argv) < 2:
        print("Usage: python tools/validate_otp.py <file1.otp> [file2.otp ...]")
        sys.exit(1)

    validators = load_schemas()
    all_ok = True
    for path in sys.argv[1:]:
        try:
            validate_otp_archive(path, validators)
        except Exception as e:
            print(f"FAIL: {path}: {e}", file=sys.stderr)
            all_ok = False

    sys.exit(0 if all_ok else 1)

if __name__ == "__main__":
    main()
