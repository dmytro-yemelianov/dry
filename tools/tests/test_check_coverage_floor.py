import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "check_coverage_floor.py"


class CoverageFloorTests(unittest.TestCase):
    def run_checker(self, lcov: str, floor: str = "85.00") -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "lcov.info"
            path.write_text(lcov, encoding="utf-8")
            return subprocess.run(
                [
                    sys.executable,
                    str(CHECKER),
                    "--lcov",
                    str(path),
                    "--minimum-line-percent",
                    floor,
                ],
                text=True,
                capture_output=True,
                check=False,
            )

    def test_exact_floor_passes_across_records(self) -> None:
        result = self.run_checker(
            "SF:a.rs\nLF:80\nLH:68\nend_of_record\n"
            "SF:b.rs\nLF:20\nLH:17\nend_of_record\n"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("85.00% (85/100); required 85.00%", result.stdout)

    def test_below_floor_fails_without_rounding_up(self) -> None:
        result = self.run_checker("SF:a.rs\nLF:2001\nLH:1700\nend_of_record\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("coverage floor not met", result.stderr)

    def test_rejects_invalid_floor(self) -> None:
        for floor in ("-1", "101", "85.001", "nan"):
            with self.subTest(floor=floor):
                self.assertEqual(self.run_checker("SF:a\nLF:1\nLH:1\nend_of_record\n", floor).returncode, 2)

    def test_rejects_malformed_or_inconsistent_records(self) -> None:
        cases = {
            "non_integer": "SF:a\nLF:nope\nLH:1\nend_of_record\n",
            "negative": "SF:a\nLF:-1\nLH:0\nend_of_record\n",
            "duplicate": "SF:a\nLF:1\nLF:1\nLH:1\nend_of_record\n",
            "missing": "SF:a\nLF:1\nend_of_record\n",
            "hit_exceeds_found": "SF:a\nLF:1\nLH:2\nend_of_record\n",
            "dangling": "SF:a\nLF:1\nLH:1\n",
            "outside": "LF:1\n",
        }
        for name, lcov in cases.items():
            with self.subTest(name=name):
                self.assertEqual(self.run_checker(lcov).returncode, 2)

    def test_rejects_zero_instrumented_lines(self) -> None:
        result = self.run_checker("SF:a\nLF:0\nLH:0\nend_of_record\n", "0")
        self.assertEqual(result.returncode, 2)
        self.assertIn("zero instrumented lines", result.stderr)


if __name__ == "__main__":
    unittest.main()
