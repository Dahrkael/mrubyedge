#!/usr/bin/env python3
"""Run the vendored mruby suite and write Markdown (default), JSON or HTML."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import html
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "mrubyedge/tests/mruby_spec"
STATUSES = ("pass", "fail", "error", "warn", "skip", "stress", "interrupted")
NOTE = ("Counts include only reached assert blocks (including nested blocks), not individual "
        "assert_equal calls. Interrupted files may have further unevaluated tests; their count "
        "is unknown. Conditional tests not entered by this build are not counted as skips.")


def read_events(path):
    events = []
    if path.exists():
        for line in path.read_text(errors="replace").splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                # A killed process can leave its last record partially written.
                break
    return events


def summarize(events, returncode, timed_out, log):
    assertions, by_id, stack = [], {}, []
    phase = None
    completed = False
    for event in events:
        kind = event[0]
        if kind == "phase":
            phase = event[1]
            completed |= phase == "completed"
        elif kind == "start":
            item = {"id": event[1], "name": event[2], "parent_id": stack[-1] if stack else None,
                    "status": "interrupted", "details": []}
            assertions.append(item)
            by_id[item["id"]] = item
            stack.append(item["id"])
        elif kind == "check_failure":
            if stack:
                detail = " ".join(str(v) for v in event[2:] if v is not None)
                by_id[stack[-1]]["details"].append(detail or "assertion failed")
        elif kind in STATUSES and event[1] in by_id:
            item = by_id[event[1]]
            item["status"] = kind
            if len(event) > 2 and event[2] is not None:
                item["details"].append(str(event[2]))
            if event[1] in stack:
                stack.remove(event[1])
    if timed_out:
        status = "timeout"
    elif returncode is not None and returncode < 0:
        status = "aborted"
    elif returncode == 0 and completed and not stack:
        status = "passed"
    elif completed and any(a["status"] in ("fail", "error") for a in assertions):
        status = "failed"
    elif phase == "compile":
        status = "compile_error"
    elif phase == "returned":
        status = "premature_return"
    elif phase == "runtime_error":
        status = "runtime_error"
    elif "panicked at" in log:
        status = "panic"
    else:
        status = "incomplete"
    counts = Counter(a["status"] for a in assertions)
    return {"status": status, "completed": completed, "returncode": returncode,
            "counts": {s: counts[s] for s in STATUSES}, "assertions": assertions}


def md(value):
    # Escape HTML as well as Markdown so upstream descriptions cannot inject markup.
    value = html.escape(str(value)).replace("\n", "<br>")
    for char in "\\`*_[]|":
        value = value.replace(char, "\\" + char)
    return value


def markdown(report):
    meta = report["metadata"]
    lines = ["# mruby compatibility report", "", f"Commit: `{meta['commit']}`; dirty: `{meta['dirty']}`",
             f"Generated: {meta['generated_at']}",
             f"Features: `{meta['features'] or '(none requested)'}`; default features: `{meta['default_features']}`",
             f"Upstream: `{meta['upstream_revision']}`", "", NOTE, "",
             f"Files: {len(report['files'])}; passed: {report['file_counts'].get('passed', 0)}", "",
             "| File | Status | Pass | Fail | Error | Warn | Skip | Stress | Interrupted |",
             "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    for f in report["files"]:
        counts = " | ".join(str(f["counts"][s]) for s in STATUSES)
        lines.append(f"| [{md(f['file'])}](#{Path(f['file']).stem}) | {f['status']} | {counts} |")
    for f in report["files"]:
        lines += ["", f"## {Path(f['file']).stem}", "",
                  f"Status: **{f['status']}**; elapsed: {f['elapsed_seconds']:.2f}s; "
                  f"[source]({quote(f['source'], safe='/:' )}) · [log]({quote(f['log'])}) · [events]({quote(f['events'])})", ""]
        if not f["completed"]:
            lines += ["**Execution stopped before completion. Remaining tests are unevaluated.**", ""]
        lines += ["| ID | Parent | Assert | Result | Details |", "|---:|---:|---|---|---|"]
        for a in f["assertions"]:
            lines.append(f"| {a['id']} | {a['parent_id'] or ''} | {md(a['name'])} | {a['status']} | "
                         f"{md(chr(10).join(a['details']))} |")
    return "\n".join(lines) + "\n"


def html_report(report):
    esc = lambda value: html.escape(str(value), quote=True)
    meta = report['metadata']
    files = report['files']
    counts = {s: sum(f['counts'][s] for f in files) for s in STATUSES}
    total = sum(counts.values())
    passed = sum(f['status'] == 'passed' for f in files)
    stopped = sum(not f['completed'] for f in files)
    labels = {'pass': 'Passed', 'fail': 'Failed', 'error': 'Errors', 'warn': 'Warnings',
              'skip': 'Skipped', 'stress': 'Stress', 'interrupted': 'Interrupted'}

    def bar(values):
        denominator = sum(values.values())
        description = ', '.join(f'{labels[s]}: {values[s]}' for s in STATUSES)
        segments = ''.join(f'<span class="segment {s}" style="width:{values[s] / denominator * 100:.4f}%"></span>'
                           for s in STATUSES if values[s]) if denominator else ''
        return f'<div class="bar" role="img" aria-label="{esc(description)}">{segments}</div>'

    assets = Path(__file__).with_name('mruby_spec_report')
    css = (assets / 'report.css').read_text()
    js = (assets / 'report.js').read_text()
    rate = f'{counts["pass"] / total * 100:.1f}%' if total else '—'
    parts = ['<!doctype html><html lang="en"><head><meta charset="utf-8">',
             '<meta name="viewport" content="width=device-width,initial-scale=1">',
             f'<title>mruby/edge · Compatibility report</title><style>{css}</style></head><body>',
             '<main><header><div class="eyebrow">mruby/edge / Compatibility</div>',
             '<h1>mruby compatibility report</h1>',
             '<p class="intro">Explore the upstream mruby suite, from file coverage to individual assertions.</p>',
             '<div class="metadata">',
             f'<span>mruby/edge <a href="https://github.com/mrubyedge/mrubyedge/commit/{esc(quote(meta["commit"], safe=""))}"><code>{esc(meta["commit"])}</code></a></span>',
             f'<span>Working tree: {"modified" if meta["dirty"] else "clean"}</span>',
             f'<span>Generated {esc(meta["generated_at"])}</span></div>',
             '<details class="configuration"><summary>Build configuration</summary>',
             f'<p>Features: {esc(meta["features"] or "(none requested)")} · Default features: {esc(meta["default_features"])}</p>',
             f'<p>Upstream revision: <code>{esc(meta["upstream_revision"])}</code></p></details></header>',
             '<section class="overview" aria-label="Overall results">',
             f'<div class="metric"><span>Files passing</span><strong>{passed}<small> / {len(files)}</small></strong>',
             f'<progress max="{max(len(files), 1)}" value="{passed}" aria-label="Passing files"></progress><p>Files completed without failures</p></div>',
             f'<div class="metric"><span>Reached assertions passing</span><strong>{rate}</strong><p>{counts["pass"]:,} of {total:,} reached assertions</p></div>',
             f'<div class="metric"><span>Execution stopped</span><strong>{stopped}<small> files</small></strong><p>Remaining assertion count is unknown</p></div></section>',
             '<section class="distribution" aria-label="Assertion results"><h2>Assertion outcomes</h2>', bar(counts),
             '<div class="legend">' + ''.join(f'<span><i class="{s}"></i>{labels[s]} <b>{counts[s]:,}</b></span>' for s in STATUSES) + '</div>',
             f'<p class="note">{esc(NOTE)} Percentages describe reached assertions, not total suite coverage.</p></section>',
             '<section aria-labelledby="files-heading"><div class="section-heading"><h2 id="files-heading">Spec files</h2>',
             f'<span id="visible-count" role="status">{len(files)} files</span></div>',
             '<div class="filters" hidden><label>Find a file<input id="file-search" type="search" placeholder="e.g. array.rb"></label>',
             '<label>File results<select id="file-filter"><option value="all">All files</option><option value="passed">Passed</option>',
             '<option value="failures">Assertion failures / errors</option><option value="stopped">Execution stopped</option>',
             '<option value="attention">Needs attention</option></select></label>',
             '<label class="checkbox"><input id="assert-filter" type="checkbox">Show only non-passing assertions</label></div>',
             '<p id="empty-state" hidden>No files match these filters.</p>']
    for f in files:
        n = sum(f['counts'].values())
        ratio = f'{f["counts"]["pass"] / n * 100:.1f}%' if n else '—'
        parts.append(f'<details class="file" id="{esc(Path(f["file"]).stem)}" data-name="{esc(f["file"].lower())}" '
                     f'data-status="{esc(f["status"])}" data-stopped="{str(not f["completed"]).lower()}" '
                     f'data-failures="{str(bool(f["counts"]["fail"] + f["counts"]["error"])).lower()}">'
                     f'<summary><span class="filename">{esc(f["file"])}</span><span class="badge {esc(f["status"])}">{esc(f["status"].replace("_", " "))}</span>'
                     f'<span class="file-progress">{bar(f["counts"])}<span>{ratio} · {f["counts"]["pass"]}/{n} reached</span></span></summary>')
        parts.append(f'<div class="file-body"><p class="file-links">{f["elapsed_seconds"]:.2f}s · ' + ' · '.join(
            f'<a href="{esc(quote(f[key], safe="/:"))}">{key}</a>' for key in ('source', 'log', 'events')) + '</p>')
        parts.append('<div class="legend">' + ''.join(
            f'<span><i class="{s}"></i>{labels[s]} <b>{f["counts"][s]}</b></span>'
            for s in STATUSES) + '</div>')
        if not f['completed']:
            parts.append('<p class="notice">Execution stopped before completion. Remaining tests are unevaluated; their count is unknown.</p>')
        parts.append('<div class="table-scroll"><table><thead><tr><th>ID / Parent</th><th>Assert</th><th>Result</th><th>Details</th></tr></thead><tbody>')
        for a in f['assertions']:
            parts.append(f'<tr data-result="{esc(a["status"])}"><td>{a["id"]}'
                         f'{" / " + str(a["parent_id"]) if a["parent_id"] else ""}</td><td>{esc(a["name"])}</td>'
                         f'<td><span class="badge {esc(a["status"])}">{esc(a["status"])}</span></td>'
                         f'<td class="detail">{esc(chr(10).join(a["details"]))}</td></tr>')
        parts.append('</tbody></table></div><p class="no-assertions" hidden>No assertions match this view.</p></div></details>')
    parts.append(f'</section><footer>mruby/edge · Upstream compatibility report</footer></main><script type="module">{js}</script></body></html>')
    return '\n'.join(parts) + '\n'

def build(args):
    command = ['cargo', 'test', '-p', 'mrubyedge', '--test', 'mruby_spec', '--no-run', '--message-format=json']
    if args.features:
        command += ['--features', args.features]
    if args.no_default_features:
        command += ['--no-default-features']
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, text=True)
    if result.returncode:
        raise RuntimeError('cargo build failed; no compatibility report generated')
    for line in result.stdout.splitlines():
        item = json.loads(line)
        if item.get('reason') == 'compiler-artifact' and item['target']['name'] == 'mruby_spec' and item.get('executable'):
            return item['executable']
    raise RuntimeError('cargo did not return the test executable')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--format', choices=['markdown', 'json', 'html'], default='markdown')
    parser.add_argument('--output', type=Path, help='default: target/mruby-spec-report.{md,json,html}')
    parser.add_argument('--file', action='append', help='run only this upstream file (repeatable, .rb optional)')
    parser.add_argument('--source-url', help='HTTP(S) base URL for links to upstream source files')
    parser.add_argument('--features', default='', help='Cargo features, comma separated')
    parser.add_argument('--no-default-features', action='store_true')
    parser.add_argument('--timeout', type=float, default=30, help='seconds per file (default: 30)')
    args = parser.parse_args()
    if args.source_url and not args.source_url.startswith(('https://', 'http://')):
        parser.error('--source-url must be an HTTP(S) URL')
    if not 0 < args.timeout < float('inf'):
        parser.error('--timeout must be finite and positive')
    files = re.findall(r'(spec_\w+)\s*=>\s*"([^"]+)"', (SUITE.parent / 'mruby_spec.rs').read_text())
    if args.file:
        selected = {f if f.endswith('.rb') else f + '.rb' for f in args.file}
        unknown = selected - {f for _, f in files}
        if unknown:
            parser.error('unknown files: ' + ', '.join(sorted(unknown)))
        files = [(test, f) for test, f in files if f in selected]
    extension = {'markdown': 'md', 'json': 'json', 'html': 'html'}[args.format]
    output = (args.output or ROOT / f'target/mruby-spec-report.{extension}').resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    binary = build(args)
    logs = Path(tempfile.mkdtemp(prefix='mruby-spec-logs-', dir=output.parent))
    git = lambda *cmd: subprocess.check_output(['git', *cmd], cwd=ROOT, text=True).strip()
    revision = re.search(r'full source revision is `([0-9a-f]+)`', (SUITE / 'README.md').read_text())[1]
    report = {'schema_version': 1, 'metadata': {
        'commit': git('rev-parse', 'HEAD'), 'dirty': bool(git('status', '--porcelain')),
        'generated_at': datetime.now(timezone.utc).isoformat(), 'features': args.features,
        'default_features': not args.no_default_features, 'upstream_revision': revision,
        'timeout_seconds': args.timeout, 'note': NOTE,
    }, 'files': []}
    for test, filename in files:
        events = logs / (filename + '.jsonl')
        log = logs / (filename + '.log')
        events.touch()
        env = os.environ.copy()
        env.update(MRUBYEDGE_SPEC_CHILD=test, MRUBYEDGE_SPEC_EVENTS=str(events))
        start = time.monotonic()
        timed_out = False
        with log.open('w') as stream:
            try:
                result = subprocess.run([binary, '--ignored', '--exact', test, '--nocapture'],
                                        cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=args.timeout)
                returncode = result.returncode
            except subprocess.TimeoutExpired:
                timed_out, returncode = True, None
        item = summarize(read_events(events), returncode, timed_out, log.read_text(errors='replace'))
        item.update(file=filename, elapsed_seconds=round(time.monotonic() - start, 3),
                    log=os.path.relpath(log, output.parent), events=os.path.relpath(events, output.parent),
                    source=(args.source_url.rstrip('/') + '/' + quote(filename) if args.source_url else
                            os.path.relpath(SUITE / 'upstream/t' / filename, output.parent)))
        report['files'].append(item)
        print(f'{filename}: {item["status"]}', file=sys.stderr)
    report['file_counts'] = dict(Counter(f['status'] for f in report['files']))
    report['assert_counts'] = {s: sum(f['counts'][s] for f in report['files']) for s in STATUSES}
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + '\n' if args.format == 'json' else (
        markdown(report) if args.format == 'markdown' else html_report(report))
    output.write_text(rendered, encoding='utf-8')
    print(f'Report: {output}', file=sys.stderr)
    return int(any(f['status'] != 'passed' for f in report['files']))


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f'error: {error}', file=sys.stderr)
        sys.exit(2)
