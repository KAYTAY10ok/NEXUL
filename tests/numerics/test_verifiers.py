"""Regression tests: malformed numerical output must never pass verification."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent

class Verifiers(unittest.TestCase):
    def verify(self, script, *contents):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for i, content in enumerate(contents):
                path = Path(directory) / str(i)
                path.write_text(content)
                paths.append(str(path))
            return subprocess.run([sys.executable, '-I', str(HERE / script), *paths], capture_output=True).returncode

    def test_empty_format(self):
        self.assertNotEqual(self.verify('format_verify.py', ''), 0)

    def test_format_error_marker(self):
        self.assertNotEqual(self.verify('format_verify.py', 'BAD-ROUNDTRIP 0 0\n'), 0)

    def test_empty_math(self):
        self.assertNotEqual(self.verify('math_verify.py', ''), 0)

    def test_missing_math_columns(self):
        self.assertNotEqual(self.verify('math_verify.py', '0 0\n'), 0)

    def test_extra_parse_output(self):
        self.assertNotEqual(self.verify('parse_compare.py', '0\n', '0\n', '0\nextra\n'), 0)

    def test_empty_parse(self):
        self.assertNotEqual(self.verify('parse_compare.py', '', '', ''), 0)

    def test_valid_parse(self):
        self.assertEqual(self.verify('parse_compare.py', '0\n', '0\n', '0\n'), 0)

if __name__ == '__main__':
    unittest.main()
