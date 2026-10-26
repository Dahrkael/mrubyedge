#!/usr/bin/env python3
"""Plot a completed run.py JSON report as PNG, SVG or PDF."""
import argparse
import json
import math
from pathlib import Path
import re
import textwrap


def positive(value, description):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
        raise ValueError(f"{description} must be a positive finite number")
    return value


def panels(report, cases=None, mode="both"):
    if not isinstance(report, dict) or report.get("schema_version") != 1 or report.get("status") != "complete":
        raise ValueError("expected a complete schema_version 1 benchmark report")
    versions = report.get("versions")
    if not isinstance(versions, list) or not versions:
        raise ValueError("report has no versions")
    if any(not isinstance(v, dict) or not isinstance(v.get("results"), dict) for v in versions):
        raise ValueError("each version must contain a results object")
    available = list(versions[0].get("results", {}))
    selected = list(dict.fromkeys(cases or [key.rsplit("/", 1)[0] for key in available]))
    unknown = set(selected) - {key.rsplit("/", 1)[0] for key in available}
    if unknown:
        raise ValueError("unknown workloads: " + ", ".join(sorted(unknown)))
    output = []
    for case in selected:
        for current in ("cold", "warm") if mode == "both" else (mode,):
            key = f"{case}/{current}"
            if mode == "both" and not any(key in v.get("results", {}) for v in versions):
                continue
            values = []
            for version in versions:
                result = version.get("results", {}).get(key)
                if not isinstance(result, dict):
                    raise ValueError(f"missing {key} for {version.get('ref', 'unknown version')}")
                if result.get("unit") != "ns/iteration":
                    raise ValueError(f"unsupported measurement unit for {key}")
                median = positive(result.get("median"), f"{key} median")
                raw = result.get("samples")
                if not isinstance(raw, list) or not raw:
                    raise ValueError(f"missing raw samples for {key}")
                if any(not isinstance(s, dict) for s in raw):
                    raise ValueError(f"invalid raw samples for {key}")
                samples = [positive(s.get("elapsed_ns"), f"{key} elapsed_ns") /
                           positive(s.get("iterations"), f"{key} iterations") for s in raw]
                if not min(samples) <= median <= max(samples):
                    raise ValueError(f"{key} median is outside the observed sample range")
                values.append({"median": median, "min": min(samples), "max": max(samples),
                               "count": len(samples)})
            output.append({"case": case, "mode": current, "values": values})
    if not output:
        raise ValueError("no measurements selected")
    return output


def version_labels(versions):
    labels = []
    for version in versions:
        ref = str(version.get("ref", "unknown"))
        label = ref[:10] if re.fullmatch(r"[0-9a-fA-F]{40,64}", ref) else ref
        if len(label) > 32:
            label = label[:29] + "..."
        if version.get("dirty"):
            label += " (dirty)"
        original = label
        suffix = 2
        while label in labels:
            label = f"{original} [{suffix}]"
            suffix += 1
        labels.append(label)
    return labels


def render(report, selected, metric, output):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        from matplotlib.ticker import MaxNLocator
    except ImportError as error:
        raise ValueError("plotting needs matplotlib; install benchmarks/requirements-plot.txt") from error
    columns = min(2, len(selected))
    rows = math.ceil(len(selected) / columns)
    panel_height = max(2.7, .45 * len(report["versions"]) + 1.2)
    fig, axes = plt.subplots(rows, columns, squeeze=False,
                             figsize=(7 * columns, panel_height * rows + 1.7),
                             layout="constrained")
    labels = version_labels(report["versions"])
    colors = ["#64748B"] + ["#168774"] * (len(labels) - 1)
    for panel, ax in zip(selected, axes.flat):
        values = panel["values"]
        if metric == "time":
            numbers = [v["median"] / 1e6 for v in values]
            lower = [(v["median"] - v["min"]) / 1e6 for v in values]
            upper = [(v["max"] - v["median"]) / 1e6 for v in values]
            endpoints = [v["max"] / 1e6 for v in values]
            errors = [lower, upper]
            ax.set_xlabel("Median time per invocation (ms); lower is faster")
        else:
            numbers = [values[0]["median"] / v["median"] for v in values]
            endpoints = numbers
            errors = None
            ax.axvline(1, color="#94A3B8", linewidth=1)
            ax.set_xlabel(f"Speedup vs {labels[0]}; higher is faster")
        ax.barh(range(len(labels)), numbers, height=.55, color=colors, xerr=errors,
                error_kw={"ecolor": "#334155", "elinewidth": 1, "capsize": 3}, zorder=3)
        maximum = max(endpoints)
        for index, (number, endpoint) in enumerate(zip(numbers, endpoints)):
            text = f"{number:.3f}" if metric == "time" else f"{number:.3f}x"
            ax.text(endpoint + maximum * .025, index, text, va="center", fontsize=10)
        ax.set_yticks(range(len(labels)), labels)
        ax.invert_yaxis()
        ax.set_xlim(0, maximum * 1.28)
        ax.xaxis.set_major_locator(MaxNLocator(nbins=5))
        ax.set_title(f'{panel["case"]} / {panel["mode"]}', loc="left", pad=12)
        ax.grid(axis="x", color="#E2E8F0", linewidth=.7)
        ax.set_axisbelow(True)
        for edge in ("top", "right", "left"):
            ax.spines[edge].set_visible(False)
    for ax in list(axes.flat)[len(selected):]:
        ax.set_visible(False)
    config = report.get("config", {})
    counts = sorted({v["count"] for p in selected for v in p["values"]})
    title = "mruby/edge: native VM " + ("execution time" if metric == "time" else "speedup")
    details = (f'profile: {config.get("profile", "unknown")} | '
               f'default features: {config.get("default_features", "unknown")} | '
               f'features: {", ".join(config.get("features", [])) or "none"} | '
               f'samples: {", ".join(map(str, counts))}')
    note = ("Whiskers: observed sample min/max, not confidence intervals. "
            if metric == "time" else "Ratios use median times, not significance tests. ")
    width = 75 if columns == 1 else 120
    fig.suptitle(title + "\n" + textwrap.fill(details, width) + "\n" +
                 textwrap.fill(note + "Panels use independent linear scales.", width), fontsize=11)
    output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output, dpi=160, facecolor="white")
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="results.json from run.py")
    parser.add_argument("--output", type=Path, help="PNG, SVG or PDF path (default: beside the input JSON)")
    parser.add_argument("--metric", choices=["time", "speedup"], default="time")
    parser.add_argument("--mode", choices=["cold", "warm", "both"], default="both")
    parser.add_argument("--case", action="append", help="workload name; repeat to select multiple")
    args = parser.parse_args()
    output = args.output or args.report.with_name(f"{args.report.stem}-{args.metric}.png")
    if output.suffix.lower() not in (".png", ".svg", ".pdf"):
        parser.error("--output must have a .png, .svg or .pdf extension")
    try:
        report = json.loads(args.report.read_text())
        selected = panels(report, args.case, args.mode)
        render(report, selected, args.metric, output)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Plot failed: {error}\n")
    print(output.resolve())


if __name__ == "__main__":
    main()
