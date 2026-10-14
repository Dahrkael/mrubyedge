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
    meta = report["metadata"]
    parts = ['<!doctype html><html lang="en"><meta charset="utf-8">',
             '<meta name="viewport" content="width=device-width,initial-scale=1">',
             '<title>mruby compatibility report</title><style>',
             'body{font:15px system-ui;margin:2rem auto;padding:0 1rem;max-width:1200px;color:#17202a}',
             'table{border-collapse:collapse;width:100%;margin:1rem 0}th,td{padding:.5rem;border:1px solid #ccd;text-align:left;vertical-align:top}',
             'th{background:#eef2f6}.pass,.passed{color:#166534}.fail,.failed,.error,.panic,.aborted,.timeout,.interrupted{color:#b42318}',
             'details{margin:1rem 0}summary{cursor:pointer;font-weight:bold}.detail{white-space:pre-wrap;overflow-wrap:anywhere}',
             '</style><body><h1>mruby compatibility report</h1>',
             f'<p>Commit: <code>{esc(meta["commit"])}</code> · dirty: {esc(meta["dirty"])}</p>',
             f'<p>Generated: {esc(meta["generated_at"])} · Features: {esc(meta["features"] or "(none requested)")} · '
             f'default features: {esc(meta["default_features"])}</p>',
             f'<p>Upstream: <code>{esc(meta["upstream_revision"])}</code></p>', f'<p>{esc(NOTE)}</p>',
             f'<p>Files: {len(report["files"])}; passed: {report["file_counts"].get("passed", 0)}</p>',
             '<table><thead><tr><th>File</th><th>Status</th>' + ''.join(f'<th>{s.title()}</th>' for s in STATUSES) + '</tr></thead><tbody>']
    for f in report["files"]:
        parts.append(f'<tr><td><a href="#{esc(Path(f["file"]).stem)}">{esc(f["file"])}</a></td>'
                     f'<td class="{esc(f["status"])}">{esc(f["status"])}</td>' +
                     ''.join(f'<td>{f["counts"][s]}</td>' for s in STATUSES) + '</tr>')
    parts.append('</tbody></table>')
    for f in report["files"]:
        parts.append(f'<details id="{esc(Path(f["file"]).stem)}"><summary>{esc(f["file"])} — {esc(f["status"])}</summary>')
        parts.append(f'<p>Elapsed: {f["elapsed_seconds"]:.2f}s · ' + ' · '.join(
            f'<a href="{esc(quote(f[key], safe="/:"))}">{key}</a>' for key in ('source', 'log', 'events')) + '</p>')
        if not f["completed"]:
            parts.append('<p><strong>Execution stopped before completion. Remaining tests are unevaluated.</strong></p>')
        parts.append('<table><thead><tr><th>ID</th><th>Parent</th><th>Assert</th><th>Result</th><th>Details</th></tr></thead><tbody>')
        for a in f["assertions"]:
            parts.append(f'<tr><td>{a["id"]}</td><td>{a["parent_id"] or ""}</td><td>{esc(a["name"])}</td>'
                         f'<td class="{esc(a["status"])}">{esc(a["status"])}</td><td class="detail">{esc(chr(10).join(a["details"]))}</td></tr>')
        parts.append('</tbody></table></details>')
    return '\n'.join(parts) + '</body></html>\n'


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
