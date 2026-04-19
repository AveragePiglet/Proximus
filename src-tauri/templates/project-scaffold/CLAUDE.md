# Project Memory Protocol

This project uses a graph-based memory system at `.claude-memory/`.

## Session start
1. Read `.claude-memory/MANIFEST.toml` — vocab and load order
2. Read `.claude-memory/invariants.toml` — absolute rules
3. Read `.claude-memory/state.toml` — current task
4. Read `.claude-memory/graph.toml` — L0/L1 summaries of all nodes
5. Load `nodes/<name>.toml` only when working in that domain

## Symbolic vocabulary
See `[vocab]` in MANIFEST.toml. Use these symbols when writing to memory:
`→ ⊥ ⊤ ? ! ~ @ #`

## End of task protocol
1. Update `graph.toml`: bump `last_touched` on touched nodes, add new nodes/edges
2. Update `state.toml`: reset `active_task`, set `next_action`
3. If node L2 content changed significantly, update `nodes/<name>.toml`
4. Append 3-5 line entry to `journal/<YYYY-WW>.toml`
5. Run `python .claude-memory/tools/validate.py` — must pass
6. If validation fails, fix before considering task complete

## Hard rules
- node files max 120 lines (enforced by validator)
- no prose paragraphs in memory files — lists and structured TOML only
- no duplicating facts across files — reference by ID instead
- no adding nodes without at least one edge
- every bug, invariant, decision, task gets a stable ID
