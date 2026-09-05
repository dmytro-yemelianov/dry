from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "check_hosted_verification_topology.py"
SPEC = importlib.util.spec_from_file_location("check_hosted_verification_topology", CHECKER)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class HostedVerificationTopologyTests(unittest.TestCase):
    def test_normalizes_supported_pages_function_variants(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            functions = Path(directory) / "functions"
            cases = {
                "api/verify.ts": "/api/verify",
                "api/verify.js": "/api/verify",
                "api/verify/index.ts": "/api/verify",
                "api/mcp/index.js": "/api/mcp",
                "api/machines.tsx": "/api/machines",
            }
            for relative, expected in cases.items():
                source = functions / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                source.touch()
                with self.subTest(relative=relative):
                    self.assertEqual(checker.pages_function_route(functions, source), expected)

    def test_ignores_unsupported_and_outside_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            functions = root / "functions"
            unsupported = functions / "api" / "verify.md"
            outside = root / "verify.ts"
            unsupported.parent.mkdir(parents=True)
            unsupported.touch()
            outside.touch()
            self.assertIsNone(checker.pages_function_route(functions, unsupported))
            self.assertIsNone(checker.pages_function_route(functions, outside))

    def test_rejects_file_and_directory_index_forms(self) -> None:
        for relative in ("api/verify.js", "api/verify/index.ts", "api/mcp.tsx", "api/mcp/index.jsx"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                functions = Path(directory) / "functions"
                source = functions / relative
                source.parent.mkdir(parents=True, exist_ok=True)
                source.touch()
                with self.assertRaisesRegex(AssertionError, "retired hosted-verification ingress"):
                    checker.forbid_retired_pages_routes(functions)


if __name__ == "__main__":
    unittest.main()
