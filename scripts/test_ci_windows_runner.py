import argparse
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_windows_runner as runner


def pe_image(machine, offset=0x80):
    header = bytearray(offset + 24)
    header[:2] = b"MZ"
    struct.pack_into("<I", header, 0x3C, offset)
    header[offset:offset + 4] = b"PE\0\0"
    struct.pack_into("<H", header, offset + 4, machine)
    return bytes(header)


class WindowsRunnerTest(unittest.TestCase):
    def test_pe_machine_names_windows_architectures(self):
        with tempfile.TemporaryDirectory() as directory:
            for machine, name in ((0xAA64, "arm64"), (0x8664, "x64"), (0xA641, "arm64ec"), (0x014C, "x86"),
                                  (0x1234, "machine 0x1234")):
                path = Path(directory, f"{machine}.exe")
                path.write_bytes(pe_image(machine))
                self.assertEqual(runner.pe_machine(path), name)

    def test_pe_machine_rejects_other_files(self):
        with tempfile.TemporaryDirectory() as directory:
            elf = Path(directory, "tool")
            elf.write_bytes(b"\x7fELF" + bytes(60))
            self.assertEqual(runner.pe_machine(elf), "not a PE image")
            truncated = Path(directory, "short.exe")
            truncated.write_bytes(pe_image(0xAA64, offset=0x80)[:0x82])
            self.assertEqual(runner.pe_machine(truncated), "not a PE image")
            self.assertTrue(runner.pe_machine(Path(directory, "missing.exe")).startswith("unreadable"))

    def test_median_and_file_costs(self):
        self.assertEqual(runner.median_ms([0.003, 0.001, 0.002]), 2.0)
        self.assertEqual(runner.median_ms([]), 0.0)
        with tempfile.TemporaryDirectory() as directory:
            write, remove = runner.time_file_writes(Path(directory, "files"), 5)
            self.assertGreaterEqual(write, 0.0)
            self.assertGreaterEqual(remove, 0.0)
            self.assertFalse(Path(directory, "files").exists())

    def test_llg_command_and_simulator_lookup(self):
        args = argparse.Namespace(llg="llg", top="tb", source="a.sv")
        self.assertEqual(runner.llg_command(args, Path("out")), ["llg", "--top", "tb", "--out-dir", "out", "a.sv"])
        with tempfile.TemporaryDirectory() as directory:
            self.assertIsNone(runner.find_simulator(directory))
            sim = Path(directory, "sim", "tb", "build", "bin", "sim")
            sim.parent.mkdir(parents=True)
            sim.write_bytes(b"")
            self.assertEqual(runner.find_simulator(directory), sim)


if __name__ == "__main__":
    unittest.main()
