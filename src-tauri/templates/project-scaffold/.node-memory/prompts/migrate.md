# Migration Prompt

Paste this into Claude Code from the project root:

---

I've set up a new graph-based memory system at `.node-memory/` following
the protocol in `CLAUDE.md`. I want you to migrate all existing project
memory — wherever it lives — into this new system.

Memory in real projects is scattered. Do not assume it's all in one folder.

==== PHASE 0: LOCATE EXISTING MEMORY ====

Before anything else, find where existing memory lives:

1. Read the existing CLAUDE.md (the project's original, not the new
   memory protocol one). Look for:
   - Any references to memory directories or files
   - Paths like ~/.claude/projects/*/memory/, memory/, docs/, etc.
   - Instructions mentioning where context or memory is stored
   - References to MEMORY.md or similar index files

2. Check the Claude Code auto-memory location for this project:
   - Look in ~/.claude/projects/ for a directory matching this project
   - Check for a memory/ subdirectory and/or MEMORY.md index there

3. If neither step found memory locations, ask the user:
   "I couldn't find existing memory references in your CLAUDE.md or
   the default Claude Code memory location. Do you have existing memory
   files? If so, where are they?"

4. Do NOT proceed to Phase 1 until you have confirmed the memory
   source locations (or confirmed there are none to migrate).

==== PHASE 0.5: SETTINGS CHECK ====

Check `.claude/settings.local.json` exists. If it does, check that the
`permissions.allow` array contains:
  "Bash(python .node-memory/tools/validate.py)"

If the entry is missing, add it to the allow array. If the file doesn't
exist, create it with:
```json
{
  "permissions": {
    "allow": [
      "Bash(python .node-memory/tools/validate.py)"
    ]
  }
}
```
This ensures the validator can run without prompting in later phases.

==== PHASE 1: DISCOVERY ====

First, read `.node-memory/MANIFEST.toml`, the new `CLAUDE.md` protocol,
and `.node-memory/tools/validate.py` to internalize the schema.

Then scan the repo for ALL of these memory sources, including whatever
locations you found in Phase 0. Report what you find before touching
anything:

1. Existing memory locations (from Phase 0)
   - Read and inventory every file found in the memory locations above

2. Root-level docs
   - README.md, ARCHITECTURE.md, CONTRIBUTING.md, CHANGELOG.md
   - Any other *.md at the repo root

3. The existing CLAUDE.md
   - Capture any rules, conventions, or external links it contains
   - Note what should be preserved vs. replaced by the new protocol

4. Architecture Decision Records
   - docs/adr/, decisions/, architecture/decisions/, adr/
   - Any file matching *adr*, *decision*, *rfc*

5. AI-tool config files (these often contain invariants)
   - .cursorrules, .cursor/rules/, .github/copilot-instructions.md
   - .aider.conf.yml, .continue/, AGENTS.md

6. Encoded contracts
   - openapi.yaml/json, schema.graphql, proto files
   - db/migrations/ (surface intent, not every migration)
   - Dockerfile, docker-compose.yml (deployment invariants)
   - CI configs: .github/workflows/, .gitlab-ci.yml

7. Tribal knowledge in code
   - Grep the src tree for comment markers: NOTE:, HACK:, FIXME:,
     TODO:, XXX:, WARN:, DEPRECATED:
   - Only surface ones that look architecturally significant
     (not routine TODOs)

8. External references
   - Find URLs in any of the above pointing to Notion, Linear, Jira,
     Confluence, GitHub issues/PRs, Figma, Miro
   - Do NOT fetch these. Just record them as ext refs.

Present your discovery report as a structured list:
  - source path
  - one-line summary of what's there
  - proposed fate (source for nodes / source for invariants /
    external ref / skip with reason)

Then STOP and wait for my approval of the discovery report before
proceeding. If I say "skip X" or "also check Y", adjust and re-report.

==== PHASE 2: CLASSIFICATION ====

Once I approve discovery, classify the atomic entities you found into:
  - module nodes (N.*)        — architecturally significant code units
  - bug nodes (B.*)           — known issues
  - decision nodes (D.*)      — choices made, especially from ADRs
  - invariant IDs (I.*)       — contracts that must hold
  - task nodes (T.*)          — open work items

For each, note the source(s) it came from so I can audit provenance.

Propose relationships as edges with types: calls, depends_on, runs_before,
caused_by, enforces, supersedes, blocks.

For each node produce three summary levels:
  - L0: <=3 tokens
  - L1: <=20 tokens (include @file refs where applicable)
  - L2: <=80 tokens, OR a pointer to `nodes/<name>.toml`

External links go in an `[N.<id>.ext]` subtable, grouped by source
(notion, linear, gh_issues, confluence, figma, other).

Present the full classification as a diff-style preview. STOP and wait
for approval before writing.

==== PHASE 3: WRITE ====

On approval, write:
  - graph.toml: all nodes (L0+L1) and edges
  - invariants.toml: extracted invariants
  - nodes/<name>.toml: L2 details including [ext] subtables
  - journal/<current-iso-week>.toml: migration entry listing sources
    processed and counts

Apply the symbolic vocabulary (→ ⊥ ⊤ ? ! ~ @ #). No prose paragraphs.
Structured TOML only.

==== PHASE 4: VALIDATE ====

Run `python .node-memory/tools/validate.py`. Fix any errors.
Do not consider the task done until it passes.

==== PHASE 5: MERGE CLAUDE.md ====

The project's existing CLAUDE.md likely contains project-specific rules,
conventions, and instructions that must be preserved. Do NOT replace it.

1. Read the existing CLAUDE.md in full
2. Read the memory protocol section from
   .node-memory/prompts/claude-md-protocol.md
3. Merge them:
   - Keep ALL existing project-specific rules, conventions, and instructions
   - Append the memory protocol section (Session start, Symbolic vocabulary,
     End of task protocol, Hard rules)
   - If the existing CLAUDE.md already has rules that overlap with invariants
     you extracted in Phase 2, note them but keep both — the CLAUDE.md version
     is the human-readable one, the invariants.toml version is the structured one
4. Show the proposed merged CLAUDE.md. Wait for approval before writing.

==== PHASE 6: REPORT ====

Final summary:
  - Count of nodes by type, count of edges by relation
  - Source → node mapping (which original file produced which nodes)
  - External refs captured (by service)
  - CLAUDE.md merge summary (what was kept, what was added)
  - Anything found but deliberately skipped, with reason
  - Anything ambiguous that I should review

Constraints:
- Do NOT migrate these into the graph — they are capability/template layer,
  not memory:
    * SKILL.md files or anything in skills/ directories
    * Project template files (e.g. templates/*.md, scaffolds)
    * MCP server configs
    * Generic prompt libraries not specific to this project
  If any of these are actively used BY this project, create a node that
  REFERENCES them using skill:/template:/mcp: notation. The source files
  stay where they are.
- Do not delete or modify original memory sources. Leave them in place.
- Do not fetch external URLs. Just record them.
- If unsure about classification, ask before guessing.
- Every node needs at least one edge — flag orphans.
- Node files <= 120 lines (validator enforces).

Begin with Phase 1.
