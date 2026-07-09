#!/usr/bin/env python3
"""Compare Criterion estimate JSON files between two saved baselines."""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


DEFAULT_THRESHOLD = 1.05


@dataclass(frozen=True)
class Estimate:
    benchmark: str
    point_estimate: float
    path: Path


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compare Criterion mean point estimates between a baseline and a "
            "candidate. Candidate/baseline ratios above --threshold exit non-zero."
        )
    )
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=Path("target/criterion"),
        help="Criterion output directory containing saved baseline folders.",
    )
    parser.add_argument(
        "--baseline",
        default="base",
        help="Saved Criterion baseline name for the comparison baseline.",
    )
    parser.add_argument(
        "--candidate",
        default="new",
        help="Saved Criterion baseline name for the candidate.",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=DEFAULT_THRESHOLD,
        help="Allowed candidate/baseline multiplier, for example 1.05.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run an internal filesystem/parser self-test and exit.",
    )
    parser.add_argument(
        "benchmarks",
        nargs="*",
        help="Optional benchmark names to compare. Defaults to all common names.",
    )
    return parser.parse_args(argv)


def mean_point_estimate(path: Path) -> float:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    try:
        value = payload["mean"]["point_estimate"]
    except (KeyError, TypeError) as error:
        raise ValueError(f"{path} does not contain mean.point_estimate") from error
    if not isinstance(value, (int, float)):
        raise ValueError(f"{path} mean.point_estimate is not numeric")
    value = float(value)
    if value <= 0:
        raise ValueError(f"{path} mean.point_estimate must be positive")
    return value


def benchmark_name_for_estimate(criterion_dir: Path, baseline_name: str, path: Path) -> str | None:
    relative_parts = path.relative_to(criterion_dir).parts
    try:
        baseline_index = relative_parts.index(baseline_name)
    except ValueError:
        return None
    if relative_parts[baseline_index + 1 :] != ("estimates.json",):
        return None
    return "/".join(relative_parts[:baseline_index])


def load_estimates(criterion_dir: Path, baseline_name: str) -> dict[str, Estimate]:
    estimates: dict[str, Estimate] = {}
    if not criterion_dir.exists():
        raise FileNotFoundError(f"criterion dir does not exist: {criterion_dir}")
    for path in criterion_dir.rglob("estimates.json"):
        benchmark = benchmark_name_for_estimate(criterion_dir, baseline_name, path)
        if benchmark is None:
            continue
        estimates[benchmark] = Estimate(
            benchmark=benchmark,
            point_estimate=mean_point_estimate(path),
            path=path,
        )
    return estimates


def select_benchmarks(
    baseline: dict[str, Estimate],
    candidate: dict[str, Estimate],
    requested: list[str],
) -> tuple[list[str], list[str]]:
    if requested:
        missing = [
            name
            for name in requested
            if name not in baseline or name not in candidate
        ]
        return requested, missing
    common = sorted(set(baseline).intersection(candidate))
    missing: list[str] = []
    return common, missing


def compare(
    criterion_dir: Path,
    baseline_name: str,
    candidate_name: str,
    threshold: float,
    requested: list[str],
) -> int:
    if threshold < 1.0:
        raise ValueError("--threshold must be a multiplier >= 1.0, for example 1.05")
    baseline = load_estimates(criterion_dir, baseline_name)
    candidate = load_estimates(criterion_dir, candidate_name)
    benchmarks, missing = select_benchmarks(baseline, candidate, requested)
    if missing:
        print("missing benchmark estimates:", ", ".join(missing), file=sys.stderr)
        return 2
    if not benchmarks:
        print("no common benchmark estimates found", file=sys.stderr)
        return 2

    rows: list[tuple[str, float, float, float, str]] = []
    failed = False
    for benchmark in benchmarks:
        base = baseline[benchmark].point_estimate
        cand = candidate[benchmark].point_estimate
        ratio = cand / base
        status = "FAIL" if ratio > threshold else "OK"
        failed = failed or status == "FAIL"
        rows.append((benchmark, base, cand, ratio, status))

    print("benchmark,baseline,candidate,ratio,limit,status")
    for benchmark, base, cand, ratio, status in rows:
        print(f"{benchmark},{base:.6f},{cand:.6f},{ratio:.6f},{threshold:.6f},{status}")
    return 1 if failed else 0


def write_estimate(path: Path, point_estimate: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"mean": {"point_estimate": point_estimate}}),
        encoding="utf-8",
    )


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp) / "criterion"
        write_estimate(root / "sqlite_create_task_ready" / "base" / "estimates.json", 100.0)
        write_estimate(root / "sqlite_create_task_ready" / "new" / "estimates.json", 105.0)
        write_estimate(
            root / "sqlite_claim_task_ready" / "base" / "estimates.json",
            100.0,
        )
        write_estimate(
            root / "sqlite_claim_task_ready" / "new" / "estimates.json",
            105.01,
        )
        write_estimate(
            root / "sqlite_list_tasks_page_25_of_1000" / "base" / "estimates.json",
            200.0,
        )
        ok = compare(root, "base", "new", 1.05, ["sqlite_create_task_ready"])
        fail = compare(root, "base", "new", 1.05, ["sqlite_claim_task_ready"])
        missing = compare(root, "base", "new", 1.05, ["sqlite_list_tasks_page_25_of_1000"])
        if ok != 0 or fail != 1 or missing != 2:
            print(
                f"self-test failed: ok={ok} fail={fail} missing={missing}",
                file=sys.stderr,
            )
            return 1
    print("self-test passed")
    return 0


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        if args.self_test:
            return self_test()
        return compare(
            args.criterion_dir,
            args.baseline,
            args.candidate,
            args.threshold,
            args.benchmarks,
        )
    except (FileNotFoundError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
