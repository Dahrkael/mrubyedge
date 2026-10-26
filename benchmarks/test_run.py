import argparse
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run


class BenchmarkTests(unittest.TestCase):
    def test_batch_statistics_use_per_iteration_times(self):
        result = run.summarize("0\t10\t1000\n1\t20\t4000\n", 2)
        self.assertEqual(result["median"], 150)
        self.assertEqual(result["mean"], 150)
        self.assertEqual(result["min"], 100)
        self.assertEqual(result["samples"][1], {"iterations": 20, "elapsed_ns": 4000})

    def test_incomplete_invalid_or_noisy_samples_are_rejected(self):
        for rows in ["", "0\t1\t100", "0\t0\t1\n1\t1\t100",
                     "0\t1\t0\n1\t1\t100", "1\t1\t100\n0\t1\t100",
                     "hello\n0\t1\t100\n1\t1\t100", "0\t1\n1\t1\t100"]:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                run.summarize(rows, 2)

    def test_single_sample_has_zero_deviation(self):
        self.assertEqual(run.summarize("0\t2\t10", 1)["stdev"], 0)

    def test_speedup_direction(self):
        report = {"versions": [
            {"ref": "baseline", "results": {"integer/warm": {"median": 200}}},
            {"ref": "candidate", "results": {"integer/warm": {"median": 100}}},
        ]}
        self.assertIn("2.000x", run.table(report))

    def test_historical_snapshot_builds_direct_path_with_its_own_compiler(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "repo with spaces"
            crate = root / "mrubyedge"
            crate.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "mrubyedge"\nversion = "1.0.0"\n'
                '[dev-dependencies]\nmruby-compiler2-sys = "0.3.0"\n')
            run.command("git", "init", "--quiet", cwd=root)
            run.command("git", "add", ".", cwd=root)
            run.command("git", "-c", "user.name=Benchmark Test", "-c", "user.email=benchmark@example.com",
                        "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture", cwd=root)
            directory = Path(tmp) / "snapshot"
            directory.mkdir()
            options = argparse.Namespace(compiler_version=None, features=["wasi"], no_default_features=True)
            # Exercise a real archive without requiring historical tags in a shallow clone.
            with patch.object(run, "ROOT", root):
                version = run.prepare("HEAD", directory, options)
            manifest = run.tomllib.loads((Path(version["project"]) / "Cargo.toml").read_text())
            self.assertEqual(version["compiler"], "=0.3.0")
            self.assertEqual(version["crate_version"], "1.0.0")
            self.assertFalse(version["dirty"])
            self.assertEqual(manifest["dependencies"]["mrubyedge"]["path"], str(directory / "source/mrubyedge"))
            self.assertFalse(manifest["dependencies"]["mrubyedge"]["default-features"])
            self.assertEqual(manifest["dependencies"]["mrubyedge"]["features"], ["wasi"])

    def test_unknown_ref_fails_without_falling_back_to_head(self):
        failure = run.subprocess.CalledProcessError(128, ["git", "rev-parse"])
        with tempfile.TemporaryDirectory() as tmp, patch.object(run, "command", side_effect=failure) as command:
            with self.assertRaises(run.subprocess.CalledProcessError):
                run.prepare("no-such-benchmark-ref", Path(tmp), argparse.Namespace())
            command.assert_called_once()

    def test_numeric_parameters(self):
        self.assertEqual(run.positive("1"), 1)
        self.assertEqual(run.nonnegative("0"), 0)
        for converter, value in [(run.positive, "0"), (run.positive, "-1"), (run.nonnegative, "-1")]:
            with self.assertRaises(argparse.ArgumentTypeError):
                converter(value)


if __name__ == "__main__":
    unittest.main()
