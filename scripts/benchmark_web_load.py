#!/usr/bin/env python3
"""Reproducible HTTP benchmark for the kanban web server read endpoints.

Measures TTFB, total time and response size per endpoint over N runs and
reports min/median/p95/max, plus a concurrent board+dashboard scenario that is
what exposes duplicated per-request repository builds.

The server must already be running. Generate a reproducible fixture with:

    cargo test -p kanban-web-server --release -- \\
        --ignored --nocapture materialize_fixture
    ./target/release/kanban web serve --repo-root <printed path>

Then:

    python3 scripts/benchmark_web_load.py --base-url http://127.0.0.1:3000

The no-HTTP counterpart (cold read-model build, `doctor`, `validate`) lives in
`crates/web-server/src/bench.rs` and is run with:

    cargo test -p kanban-web-server --release -- \\
        --ignored --nocapture read_path_bench
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field

DEFAULT_ENDPOINTS = [
    "/api/repository",
    "/api/metrics",
    "/api/report",
    "/api/config",
    "/api/team",
    # Present only once page-specific contracts land (Phase 3, conditional).
    "/api/progress",
    "/api/board",
]


@dataclass
class Sample:
    status: int
    ttfb_ms: float
    total_ms: float
    bytes_read: int


@dataclass
class EndpointResult:
    path: str
    samples: list[Sample] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return bool(self.samples) and all(s.status == 200 for s in self.samples)

    def summary(self) -> dict[str, object]:
        totals = sorted(s.total_ms for s in self.samples)
        ttfbs = sorted(s.ttfb_ms for s in self.samples)
        return {
            "path": self.path,
            "runs": len(self.samples),
            "status": self.samples[0].status if self.samples else 0,
            "bytes": self.samples[0].bytes_read if self.samples else 0,
            "ttfb_median_ms": round(percentile(ttfbs, 0.5), 2),
            "min_ms": round(totals[0], 2) if totals else 0.0,
            "median_ms": round(percentile(totals, 0.5), 2),
            "p95_ms": round(percentile(totals, 0.95), 2),
            "max_ms": round(totals[-1], 2) if totals else 0.0,
        }


def percentile(sorted_values: list[float], fraction: float) -> float:
    if not sorted_values:
        return 0.0
    index = round((len(sorted_values) - 1) * fraction)
    return sorted_values[index]


def fetch(url: str, timeout: float) -> Sample:
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            first_byte = time.perf_counter()
            body = response.read()
            finished = time.perf_counter()
            status = response.status
    except urllib.error.HTTPError as err:
        first_byte = finished = time.perf_counter()
        body = err.read()
        status = err.code
    except urllib.error.URLError as err:
        raise SystemExit(f"cannot reach {url}: {err.reason}") from err
    return Sample(
        status=status,
        ttfb_ms=(first_byte - started) * 1000.0,
        total_ms=(finished - started) * 1000.0,
        bytes_read=len(body),
    )


def probe(base_url: str, path: str, timeout: float) -> bool:
    """Return True when the endpoint exists on this build."""
    try:
        return fetch(base_url + path, timeout).status == 200
    except SystemExit:
        raise
    except Exception:
        return False


def measure_endpoint(
    base_url: str, path: str, runs: int, warmup: int, timeout: float
) -> EndpointResult:
    url = base_url + path
    for _ in range(warmup):
        fetch(url, timeout)
    result = EndpointResult(path=path)
    for _ in range(runs):
        result.samples.append(fetch(url, timeout))
    return result


def measure_concurrent(
    base_url: str, paths: list[str], runs: int, warmup: int, timeout: float
) -> dict[str, object]:
    """Board and dashboard load together on a cold navigation. Serving them
    concurrently is what reveals whether the server builds the read model twice.
    """
    urls = [base_url + path for path in paths]

    def one_round() -> float:
        started = time.perf_counter()
        with ThreadPoolExecutor(max_workers=len(urls)) as pool:
            list(pool.map(lambda url: fetch(url, timeout), urls))
        return (time.perf_counter() - started) * 1000.0

    for _ in range(warmup):
        one_round()
    totals = sorted(one_round() for _ in range(runs))
    return {
        "path": " + ".join(paths) + " (concurrent)",
        "runs": runs,
        "status": 200,
        "bytes": 0,
        "ttfb_median_ms": 0.0,
        "min_ms": round(totals[0], 2),
        "median_ms": round(percentile(totals, 0.5), 2),
        "p95_ms": round(percentile(totals, 0.95), 2),
        "max_ms": round(totals[-1], 2),
    }


def render_table(rows: list[dict[str, object]]) -> str:
    headers = [
        ("path", "endpoint", 46),
        ("runs", "n", 4),
        ("status", "code", 5),
        ("bytes", "bytes", 9),
        ("ttfb_median_ms", "ttfb", 9),
        ("min_ms", "min", 9),
        ("median_ms", "median", 9),
        ("p95_ms", "p95", 9),
        ("max_ms", "max", 9),
    ]
    lines = ["  ".join(title.ljust(width) for _, title, width in headers)]
    lines.append("  ".join("-" * width for _, _, width in headers))
    for row in rows:
        lines.append(
            "  ".join(str(row[key]).ljust(width) for key, _, width in headers)
        )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:3000")
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument(
        "--endpoint",
        action="append",
        dest="endpoints",
        help="Override the measured endpoints (repeatable).",
    )
    parser.add_argument("--output", choices=["table", "json", "csv"], default="table")
    args = parser.parse_args()

    base_url = args.base_url.rstrip("/")
    candidates = args.endpoints or list(DEFAULT_ENDPOINTS)

    # Epic detail needs a real id; take the first one the repository reports.
    endpoints = []
    for path in candidates:
        if probe(base_url, path, args.timeout):
            endpoints.append(path)
        elif args.endpoints:
            print(f"warning: {path} did not return 200; skipping", file=sys.stderr)

    epic_path = first_epic_path(base_url, args.timeout)
    if epic_path:
        endpoints.append(epic_path)

    rows = [
        measure_endpoint(base_url, path, args.runs, args.warmup, args.timeout).summary()
        for path in endpoints
    ]

    concurrent_paths = [
        path for path in ("/api/repository", "/api/metrics") if path in endpoints
    ]
    if len(concurrent_paths) > 1:
        rows.append(
            measure_concurrent(
                base_url, concurrent_paths, args.runs, args.warmup, args.timeout
            )
        )

    if args.output == "json":
        print(json.dumps(rows, indent=2))
    elif args.output == "csv":
        writer = csv.DictWriter(sys.stdout, fieldnames=list(rows[0].keys()))
        writer.writeheader()
        writer.writerows(rows)
    else:
        print(render_table(rows))
    return 0


def first_epic_path(base_url: str, timeout: float) -> str | None:
    try:
        with urllib.request.urlopen(base_url + "/api/repository", timeout=timeout) as r:
            snapshot = json.loads(r.read())
    except Exception:
        return None
    epics = snapshot.get("epics") or []
    if not epics:
        return None
    return f"/api/epics/{epics[0]['id']}"


if __name__ == "__main__":
    raise SystemExit(main())
