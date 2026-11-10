#!/usr/bin/env python3
"""Build the pinned upstream Content Mapper host; keep it out of npm dependencies."""
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
PIN = json.loads((ROOT / "content-mapper/upstream.json").read_text())
REVISION = PIN["revision"]
SOURCE = ROOT / "target" / "content-mapper-typescript" / REVISION
OUTPUT = ROOT / "target" / "content-mapper-tools" / "tsgo"
STAMP = OUTPUT.with_suffix(".revision")


def run(*args, cwd=ROOT, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


if not OUTPUT.is_file() or not STAMP.is_file() or STAMP.read_text().strip() != REVISION:
    SOURCE.mkdir(parents=True, exist_ok=True)
    if not (SOURCE / ".git").is_dir():
        run("git", "init", str(SOURCE))
        run("git", "remote", "add", "origin", PIN["repository"], cwd=SOURCE)
    run("git", "fetch", "--depth=1", "--filter=blob:none", "origin", REVISION, cwd=SOURCE)
    run("git", "sparse-checkout", "set", "tsc", cwd=SOURCE)
    run("git", "checkout", "--detach", REVISION, cwd=SOURCE)
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=SOURCE, text=True).strip()
    if actual != REVISION:
        raise SystemExit("TypeScript checkout does not match the pinned revision")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, GOWORK="off")
    run("go", "build", "-o", str(OUTPUT), "./cmd/tsc", cwd=SOURCE / "tsc", env=env)
    STAMP.write_text(REVISION + "\n")

run(str(OUTPUT), "--version")
