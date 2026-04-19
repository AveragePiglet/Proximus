#!/usr/bin/env python3
"""Validate .claude-memory/ structure and constraints."""
import sys, os, tomllib
from pathlib import Path

root = Path(__file__).resolve().parent.parent
errors = []

def check(cond, msg):
    if not cond:
        errors.append(msg)

# Check required files
for f in ["MANIFEST.toml", "graph.toml", "state.toml", "invariants.toml"]:
    check((root / f).exists(), f"Missing {f}")

# Check node file line limits
for node_file in (root / "nodes").glob("*.toml"):
    lines = node_file.read_text(encoding="utf-8").splitlines()
    check(len(lines) <= 120, f"{node_file.name}: {len(lines)} lines (max 120)")

# Check graph.toml nodes have at least one edge
if (root / "graph.toml").exists():
    with open(root / "graph.toml", "rb") as f:
        graph = tomllib.load(f)
    node_ids = [k for k in graph if k.startswith("N.")]
    edges = graph.get("E", [])
    edge_nodes = set()
    for e in edges:
        edge_nodes.add(e.get("from", ""))
        edge_nodes.add(e.get("to", ""))
    for nid in node_ids:
        check(nid in edge_nodes, f"{nid} has no edges")

if errors:
    print("VALIDATION FAILED:")
    for e in errors:
        print(f"  {e}")
    sys.exit(1)
else:
    print("All checks passed")
    sys.exit(0)
