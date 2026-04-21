# Project Memory Protocol

This project uses a graph-based memory system at `.node-memory/`.

## Session start
1. Read `.node-memory/MANIFEST.toml` — vocab and load order
2. Read `.node-memory/invariants.toml` — absolute rules
3. Read `.node-memory/state.toml` — current task
4. Read `.node-memory/graph.toml` — L0/L1 summaries of all nodes
5. Load `nodes/<name>.toml` only when working in that domain
6. Load `plans/<name>.toml` if the task involves a plan referenced in state.toml

## Symbolic vocabulary
See `[vocab]` in MANIFEST.toml. Use these symbols when writing to memory:
`→ ⊥ ⊤ ? ! ~ @ #`

## End of task protocol
1. Update `graph.toml`: bump `last_touched` on touched nodes, add new nodes/edges
2. Update `state.toml`: reset `active_task`, set `next_action`
3. If node L2 content changed significantly, update `nodes/<name>.toml`
4. If a plan was created or updated, write/update `plans/<name>.toml`
5. Append 3-5 line entry to `journal/<YYYY-WW>.toml`
6. Run `python .node-memory/tools/validate.py` — must pass
7. If validation fails, fix before considering task complete

## Plans
- All implementation plans live in `.node-memory/plans/<name>.toml`
- no storing plans as loose files in the project root — use `.node-memory/plans/`
- Reference active plan from `state.toml` via `active_plan = "plans/<name>.toml"`
- Plan files follow the same TOML-only, no-prose rule as nodes

## Hard rules
- node files max 120 lines (enforced by validator)
- no prose paragraphs in memory files — lists and structured TOML only
- no duplicating facts across files — reference by ID instead
- no adding nodes without at least one edge
- no storing plans as loose files in the project root — use `.node-memory/plans/`
- every bug, invariant, decision, task gets a stable ID
