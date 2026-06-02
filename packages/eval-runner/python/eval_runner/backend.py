"""Invoke the genuine Rust oracle (`eval-cli`) and load its validated report.

The oracle links the REAL rule-engine / hsd / document / data-model crates; there is no mock path.
"""
from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

from .common import GateReport

# .../packages/eval-runner/python/eval_runner/backend.py
_HERE = Path(__file__).resolve()
PKG_ROOT = _HERE.parents[2]          # packages/eval-runner
REPO_ROOT = _HERE.parents[4]         # Stučka
BACKEND_DIR = REPO_ROOT / "backend"  # cargo workspace
DATASETS_DIR = PKG_ROOT / "datasets"

# gate name -> dataset directory under datasets/
DATASET_DIR = {
    "calc": "calc-50",
    "deadline": "deadline-30",
    "pii": "pii-200",
    "doc": "doc-20",
    "law": "law-200",
}


def dataset_path(gate: str) -> Path:
    return DATASETS_DIR / DATASET_DIR[gate] / "manifest.json"


def _cargo() -> str:
    # Windows ships cargo.exe; the bare name resolves on every platform via PATH.
    return os.environ.get("CARGO", "cargo")


def run_oracle(gate: str, *, release: bool = False) -> GateReport:
    """Run `cargo run -p eval-cli -- <gate> <dataset> --report <tmp>` and return the validated report.

    Raises on a missing dataset, a cargo failure, or a schema-invalid report (never silently passes).
    """
    ds = dataset_path(gate)
    if not ds.is_file():
        raise FileNotFoundError(f"dataset not found for gate '{gate}': {ds}")

    with tempfile.TemporaryDirectory() as tmp:
        report_path = Path(tmp) / f"{gate}_report.json"
        cmd = [_cargo(), "run", "-q", "-p", "eval-cli", "--bin", "stuchka-eval"]
        if release:
            cmd.append("--release")
        cmd += ["--", gate, str(ds), "--report", str(report_path)]
        proc = subprocess.run(
            cmd,
            cwd=str(BACKEND_DIR),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if proc.returncode != 0:
            raise RuntimeError(
                f"eval-cli '{gate}' failed (exit {proc.returncode}).\n"
                f"stderr:\n{proc.stderr}\nstdout:\n{proc.stdout}"
            )
        if not report_path.is_file():
            raise RuntimeError(f"eval-cli '{gate}' produced no report at {report_path}")
        raw = json.loads(report_path.read_text(encoding="utf-8"))
    return GateReport.from_dict(raw)
