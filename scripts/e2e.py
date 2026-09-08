#!/usr/bin/env python3
"""Black-box tests of the COMPILED Rust CLI, never a Python replacement."""
import argparse
import json
import os
import select
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from make_fixtures import generate

BINARY = None


class CliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.storage = tempfile.TemporaryDirectory(prefix="omsk-e2e-")
        cls.root = Path(cls.storage.name)
        cls.manifest = generate(cls.root / "fixtures")
        cls.env = {key: value for key, value in os.environ.items() if not key.startswith("OMSK_")}

    @classmethod
    def tearDownClass(cls):
        cls.storage.cleanup()

    def fixture(self, name):
        return self.root / "fixtures" / name

    def invoke(self, *args, env=None, timeout=15):
        merged = dict(self.env)
        if env:
            merged.update(env)
        return subprocess.run([str(BINARY), *map(str, args)], env=merged, capture_output=True, timeout=timeout)

    def json_scan(self, *args, expected=0, env=None):
        run = self.invoke("scan", "--quiet", "--format", "json", *args, env=env)
        self.assertEqual(run.returncode, expected, run.stderr.decode(errors="replace"))
        data = json.loads(run.stdout)
        self.assertEqual(data["exit_code"], expected)
        self.assertEqual(data["schema_version"], 1)
        return data

    def test_help_version_and_rules(self):
        self.assertEqual(self.invoke("--help").returncode, 0)
        self.assertIn(b"0.2.0", self.invoke("--version").stdout)
        rules = json.loads(self.invoke("rules", "--format", "json").stdout)
        self.assertEqual(len(rules), 8)
        self.assertEqual(len({rule["id"] for rule in rules}), 8)
        self.assertIn(b"OMSK_PROFILE=strict", self.invoke("config").stdout)

    def test_all_fixture_outcomes(self):
        for name, entry in self.manifest.items():
            with self.subTest(name=name):
                data = self.json_scan("--input-format", entry["input_format"], self.fixture(name), expected=entry["expected_exit"])
                if entry["expected_matches"] is not None:
                    self.assertEqual(data["summary"]["total_findings"], entry["expected_matches"])
                    self.assertTrue(data["complete"])
                else:
                    self.assertFalse(data["complete"])
                    self.assertEqual(data["files"][0]["status"], "error")

    def test_result_limit_preserves_count_and_failure(self):
        data = self.json_scan("--input-format", "raw", "--max-findings", "1", self.fixture("all-rules.bin"), expected=1)
        entry = data["files"][0]
        self.assertEqual((entry["total_findings"], len(entry["findings"]), entry["omitted_findings"]), (8, 1, 7))

    def test_profile_and_custom_rules(self):
        data = self.json_scan("--input-format", "raw", "--profile", "timing", self.fixture("all-rules.bin"), expected=1)
        self.assertEqual(data["summary"]["total_findings"], 2)
        data = self.json_scan("--input-format", "raw", "--deny", "syscall", self.fixture("all-rules.bin"), expected=1)
        self.assertEqual(data["policy"], {"name": "custom", "rules": ["syscall"]})
        self.assertEqual(data["summary"]["total_findings"], 1)

    def test_multiple_files_preserve_order_and_errors_win(self):
        files = [self.fixture("clean.elf"), self.fixture("violation.elf"), self.root / "missing.elf"]
        data = self.json_scan(*files, expected=2)
        self.assertEqual([entry["path"] for entry in data["files"]], list(map(str, files)))
        self.assertEqual([entry["status"] for entry in data["files"]], ["clean", "violations", "error"])
        self.assertEqual(data["summary"]["completed"], 3)
        self.assertFalse(data["complete"])

    def test_sarif_binary_offsets_and_summary(self):
        run = self.invoke("scan", "--quiet", "--format", "sarif", self.fixture("violation.elf"))
        self.assertEqual(run.returncode, 1)
        document = json.loads(run.stdout)
        self.assertEqual(document["version"], "2.1.0")
        entry = document["runs"][0]
        finding = entry["results"][0]
        self.assertEqual(finding["ruleId"], "OMSK001")
        location = finding["locations"][0]["physicalLocation"]
        self.assertEqual(location["region"], {"byteOffset": 129, "byteLength": 2})
        self.assertEqual(location["artifactLocation"]["index"], 0)
        self.assertEqual(len(entry["tool"]["driver"]["rules"]), 8)
        self.assertTrue(entry["invocations"][0]["executionSuccessful"])

    def test_sarif_errors_and_truncation_are_visible(self):
        run = self.invoke("scan", "--quiet", "--format", "sarif", "--input-format", "raw", "--max-findings", "1", self.fixture("all-rules.bin"), self.root / "missing")
        self.assertEqual(run.returncode, 2)
        entry = json.loads(run.stdout)["runs"][0]
        invocation = entry["invocations"][0]
        self.assertFalse(invocation["executionSuccessful"])
        self.assertEqual(len(entry["results"]), 1)
        self.assertTrue(any(note["level"] == "warning" for note in invocation["toolExecutionNotifications"]))
        self.assertEqual(entry["artifacts"][0]["properties"]["totalFindings"], 8)

    def test_unusual_utf8_filename_is_escaped(self):
        path = self.root / 'тест space#quote"\n\x1b.bin'
        path.write_bytes(bytes.fromhex("0f05"))
        data = self.json_scan("--input-format", "raw", path, expected=1)
        self.assertEqual(data["files"][0]["path"], str(path))
        text = self.invoke("scan", "--quiet", "--input-format", "raw", path)
        self.assertNotIn(b"\x1b", text.stdout)
        sarif = self.invoke("scan", "--quiet", "--format", "sarif", "--input-format", "raw", path)
        uri = json.loads(sarif.stdout)["runs"][0]["artifacts"][0]["location"]["uri"]
        self.assertIn("%23", uri)
        self.assertIn("%1B", uri)
        self.assertNotIn('"', uri)

    def test_invalid_arguments_fail_without_success_report(self):
        cases = [[], ["--unknown", "x"], ["--max-findings", "0", "x"], ["--max-file-bytes", "-1", "x"], ["--queue-capacity", "65", "x"], ["--deny", "", "x"], ["--format", "yaml", "x"], ["--profile", "unknown", "x"], ["--max-findings"], ["x", "x"]]
        for args in cases:
            with self.subTest(args=args):
                run = self.invoke("scan", *args)
                self.assertEqual(run.returncode, 2)
                self.assertIn(b"command_failed", run.stderr)

    def test_oversized_and_non_regular_inputs(self):
        self.json_scan("--input-format", "raw", "--max-file-bytes", "1", self.fixture("clean.bin"), expected=2)
        self.json_scan(self.root, expected=2)
        link = self.root / "symlink.bin"
        link.symlink_to(self.fixture("clean.bin"))
        self.json_scan("--input-format", "raw", link, expected=2)
        pipe = self.root / "fifo"
        os.mkfifo(pipe)
        self.json_scan("--input-format", "raw", pipe, expected=2)

    def test_environment_and_cli_precedence(self):
        data = self.json_scan("--profile", "syscalls", "--input-format", "raw", self.fixture("all-rules.bin"), expected=1, env={"OMSK_PROFILE": "timing", "OMSK_MAX_FINDINGS": "2"})
        self.assertEqual(data["policy"]["name"], "syscalls")
        self.assertEqual(data["summary"]["total_findings"], 3)
        self.assertEqual(data["summary"]["reported_findings"], 2)

    def test_config_is_explicit_and_not_executed(self):
        config = self.root / "settings.env"
        config.write_text("OMSK_PROFILE=timing\nOMSK_INPUT_FORMAT=raw\n", encoding="utf-8")
        data = self.json_scan("--config", config, self.fixture("all-rules.bin"), expected=1)
        self.assertEqual(data["summary"]["total_findings"], 2)
        config.write_text("OMSK_PROFILE=$(touch /not-executed)\n", encoding="utf-8")
        run = self.invoke("scan", "--config", config, self.fixture("clean.elf"))
        self.assertEqual(run.returncode, 2)

    def test_new_atomic_report_and_no_clobber(self):
        output = self.root / "atomic-report.json"
        run = self.invoke("scan", "--quiet", "--format", "json", "--output", output, self.fixture("violation.elf"))
        self.assertEqual(run.returncode, 1)
        self.assertEqual(run.stdout, b"")
        initial = output.read_bytes()
        self.assertEqual(json.loads(initial)["exit_code"], 1)
        retry = self.invoke("scan", "--output", output, self.fixture("clean.elf"))
        self.assertEqual(retry.returncode, 2)
        self.assertEqual(output.read_bytes(), initial)
        self.assertFalse(list(self.root.glob(".omsk-report-*.tmp")))

    def test_output_cannot_overwrite_input(self):
        path = self.fixture("clean.elf")
        original = path.read_bytes()
        run = self.invoke("scan", "--output", path, path)
        self.assertEqual(run.returncode, 2)
        self.assertEqual(path.read_bytes(), original)

    def test_broken_pipe_cannot_deadlock_worker(self):
        files = []
        for number in range(10):
            path = self.root / f"busy-{number}.bin"
            path.write_bytes(bytes.fromhex("0f05") * 5000)
            files.append(path)
        process = subprocess.Popen([str(BINARY), "scan", "--quiet", "--format", "json", "--input-format", "raw", "--queue-capacity", "1", *map(str, files)], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            process.stdout.close()
            self.assertNotEqual(process.wait(timeout=15), 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stderr.close()

    def test_graceful_sigint_and_sigterm(self):
        path = self.root / "long-scan.bin"
        path.write_bytes(b"\x90" * (32 * 1024 * 1024))
        for signum in [signal.SIGINT, signal.SIGTERM]:
            with self.subTest(signal=signum):
                output = self.root / f"signal-{int(signum)}.json"
                process = subprocess.Popen([str(BINARY), "scan", "--format", "json", "--input-format", "raw", "--output", str(output), str(path)], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    self.assertTrue(select.select([process.stderr], [], [], 10)[0], "scanner never became ready")
                    self.assertIn(b"scan_start", process.stderr.readline())
                    process.send_signal(signum)
                    stdout, stderr = process.communicate(timeout=20)
                    self.assertEqual(process.returncode, 128 + int(signum), stderr)
                    self.assertEqual(stdout, b"")
                    document = json.loads(output.read_bytes())
                    self.assertFalse(document["complete"])
                    self.assertEqual(document["exit_code"], 128 + int(signum))
                    self.assertFalse(list(self.root.glob(".omsk-report-*.tmp")))
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.communicate()


def main():
    global BINARY
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parents[1] / "target/debug/omsk")
    args = parser.parse_args()
    BINARY = args.binary.resolve()
    if not BINARY.is_file():
        parser.error(f"compiled binary not found: {BINARY}; run cargo build --offline --locked first")
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(CliTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
