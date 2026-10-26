import unittest

import plot


def report():
    def version(ref, median, samples):
        return {"ref": ref, "results": {"cart/warm": {
            "unit": "ns/iteration", "median": median,
            "samples": [{"iterations": 10, "elapsed_ns": s * 10} for s in samples],
        }}}
    return {"schema_version": 1, "status": "complete", "versions": [
        version("baseline", 200, [100, 200, 300]),
        version("candidate", 100, [90, 100, 110]),
    ]}


class PlotTests(unittest.TestCase):
    def test_single_mode_report_and_batch_normalization(self):
        result = plot.panels(report())
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["mode"], "warm")
        self.assertEqual(result[0]["values"][0], {"median": 200, "min": 100, "max": 300, "count": 3})

    def test_incomplete_or_unknown_schema_is_rejected(self):
        for key, value in [("status", "running"), ("schema_version", 2), ("versions", [])]:
            data = report()
            data[key] = value
            with self.assertRaises(ValueError):
                plot.panels(data)

    def test_missing_version_result_and_unknown_selection_are_rejected(self):
        data = report()
        data["versions"][1]["results"].clear()
        with self.assertRaises(ValueError):
            plot.panels(data)
        for cases, mode in [(["unknown"], "both"), (None, "cold")]:
            with self.assertRaises(ValueError):
                plot.panels(report(), cases, mode)

    def test_invalid_timing_data_is_rejected(self):
        for value in [float("nan"), float("inf"), 0, -1, True, None]:
            data = report()
            data["versions"][0]["results"]["cart/warm"]["median"] = value
            with self.assertRaises(ValueError):
                plot.panels(data)
        data = report()
        data["versions"][0]["results"]["cart/warm"]["samples"][0]["iterations"] = 0
        with self.assertRaises(ValueError):
            plot.panels(data)

    def test_sha_shortening_and_duplicate_labels(self):
        versions = [{"ref": "a" * 40}, {"ref": "HEAD"}, {"ref": "HEAD"},
                    {"ref": "working-tree", "dirty": True}]
        self.assertEqual(plot.version_labels(versions),
                         ["a" * 10, "HEAD", "HEAD [2]", "working-tree (dirty)"])

    def test_units_samples_and_summary_range_are_checked(self):
        for field, value in [("unit", "ms"), ("samples", []), ("median", 500)]:
            data = report()
            data["versions"][0]["results"]["cart/warm"][field] = value
            with self.assertRaises(ValueError):
                plot.panels(data)


if __name__ == "__main__":
    unittest.main()
