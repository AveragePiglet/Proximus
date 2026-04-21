# Bootstrap Prompt — Existing Scaffolded Repo

Paste this into Claude Code from the project root:

---

I've just initialized the graph-based memory system at `.node-memory/`
in a project that has some existing scaffolding but no formalized memory
yet. Help me bootstrap memory from the codebase plus my intent.

Your task:

1. Read `.node-memory/MANIFEST.toml` and `CLAUDE.md`.

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

3. Scan the repo. Give me a short inventory: directory structure,
   detected stack, entry points, config files, and anything that looks
   like a notable architectural choice (build tool, ORM, auth library, etc.).

4. Ask me, ONE AT A TIME:
   a. Which of these directories/modules matter enough to be tracked as
      nodes? (I'll confirm or edit your suggestions)
   b. What is this project meant to do? (one paragraph)
   c. Are there contracts or constraints I've already baked into the
      scaffolding that should become invariants?
   d. Any decisions already made that I don't want re-litigated later?

5. Propose:
   - Invariant list
   - Seed nodes from my confirmed modules + any decision/invariant nodes
   - Edges: at minimum, structural ones inferable from the code
     (A imports B → E: A depends_on B)
   - Journal entry for the bootstrap

6. Show proposed changes. Wait for approval. Write. Validate. Update state.toml.

Rules:
- Ground node summaries in actual code — cite @file paths in L1.
- Don't promote every file to a node. Nodes are for architecturally
  significant units, not every source file.
- If scaffolding implies an invariant (e.g. a repository pattern is in
  place), flag it and ask before adding it.
