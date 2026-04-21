# Bootstrap Prompt — Greenfield (no code yet)

Paste this into Claude Code from the project root:

---

I've just initialized the graph-based memory system at `.node-memory/`
in a new project. There is no code yet — we're starting from scratch.
Help me bootstrap the memory system from project intent.

Your task:

1. Read `.node-memory/MANIFEST.toml` and `CLAUDE.md` to confirm schema
   and protocol.

2. Check `.claude/settings.local.json` exists. If it does, ensure the
   `permissions.allow` array contains:
     "Bash(python .node-memory/tools/validate.py)"
   If the entry is missing, add it. If the file doesn't exist, create it:
   ```json
   {
     "permissions": {
       "allow": [
         "Bash(python .node-memory/tools/validate.py)"
       ]
     }
   }
   ```

3. Ask me the following questions, ONE AT A TIME, waiting for my answer
   before moving on:
   a. What is this project? (one paragraph)
   b. What's the target stack? (languages, frameworks, key libraries)
   c. What are the hard constraints or contracts you already know must
      hold? (these become invariants)
   d. What are the top 3-5 initial modules or domains you anticipate?
      (these become seed nodes)
   e. Any known decisions already made that future-you should not
      re-litigate? (these become decision nodes)

4. After I've answered all five, propose:
   - An initial invariant list (I1, I2, ...) for `invariants.toml`
   - A seed node set for `graph.toml` with L0/L1 summaries
   - Edges between seed nodes expressing anticipated relationships
   - A first journal entry in `journal/<current-week>.toml`

5. Show me proposed changes as a diff-style preview. Wait for approval.

6. On approval, write the files. Run `python .node-memory/tools/validate.py`.
   Fix any failures.

7. Update `state.toml` with:
   - active_task = "T-002" (next task after bootstrap)
   - next_action = whatever I tell you I'm starting next

Rules:
- Do not invent invariants I didn't state. If you think I'm missing one, ask.
- Keep seed nodes minimal — better to grow organically than over-specify.
- Apply the symbolic vocabulary throughout.
- Do not create any `nodes/<name>.toml` files yet — L1 in graph.toml
  is enough at bootstrap. L2 detail files are created only when a node
  is actively being worked on.
