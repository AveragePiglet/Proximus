---
name: code-review
description: Perform an in-depth, multi-stage code review using Opus for deep analytical thinking and Sonnet for implementing fixes and writing detailed feedback. Use this skill whenever the user runs `/code-review`, asks for a "deep code review", "thorough review", "production-readiness review", or any time they want a rigorous review pass on changed files, a PR, a module, or a full codebase. Trigger this even when the user just says "review my code" — prefer this skill over a casual one-pass review because it produces a far more comprehensive analysis covering correctness, design, performance, maintainability, and testing.
---

# Code Review (Opus-thinks, Sonnet-implements)

A two-model code review workflow. Opus handles deep reasoning about architecture, correctness, edge cases, and tradeoffs. Sonnet handles structured implementation work: writing the review document, applying fixes, and generating examples.

## When this skill runs

The user has typed `/code-review` or asked for a thorough code review. Your job is to orchestrate a rigorous review using two models via the Task tool, then present the results.

## Step 1 — Determine the review scope

Before launching anything, figure out what to review. Ask the user only if it's genuinely ambiguous; otherwise pick the most plausible scope and state the assumption.

Scope priority order:
1. Files/paths the user explicitly named
2. Uncommitted changes (`git status` + `git diff`)
3. The current branch's diff vs `main`/`master` (`git diff main...HEAD`)
4. The most recently modified files in the working directory

Run the relevant git commands yourself to gather the actual code under review. Collect file contents, not just diffs — context matters for a real review.

## Step 2 — Launch the Opus "thinker" subagent

Use the Task tool with `model: "claude-opus-4-5"` (or the latest Opus available in this environment) and `subagent_type: "general-purpose"`. The Opus agent's job is **analysis only** — it does not write the final review document or apply fixes. It produces structured findings.

Pass it the full code under review and this prompt:

> You are the senior reviewer on a deep code review. Think hard. Analyze the provided code across these dimensions and return structured findings:
>
> 1. **Correctness & bugs** — logic errors, off-by-ones, null/undefined handling, race conditions, incorrect assumptions, broken invariants.
> 2. **Edge cases** — inputs the author didn't consider: empty, huge, malformed, concurrent, unicode, timezone, negative, zero.
> 3. **Design & architecture** — coupling, cohesion, abstraction leaks, SRP violations, misplaced responsibilities, premature abstraction, missing abstraction.
> 4. **Performance** — algorithmic complexity, N+1 queries, unnecessary allocations, blocking I/O on hot paths, caching opportunities.
> 5. **Error handling** — swallowed exceptions, unclear error messages, missing failure modes, retry/timeout gaps.
> 6. **Maintainability & readability** — naming, function length, comment quality, dead code, magic numbers, inconsistency with surrounding code.
> 7. **Testing** — what's untested, what's poorly tested, what's testing implementation rather than behavior, missing edge-case tests.
> 8. **API & contract design** — backward compatibility, surprising defaults, footguns for callers.
>
> For EACH finding return a JSON-ish object with: `severity` (critical/high/medium/low/nit), `category`, `file:line`, `problem` (1-2 sentences), `why_it_matters` (1-2 sentences), `suggested_fix` (concrete, code-level if possible).
>
> Be thorough but precise. Do not pad with generic advice. If something is genuinely good, you may note it briefly under a "strengths" section, but keep that short. Use extended thinking — this review needs depth.

## Step 3 — Launch the Sonnet "implementer" subagent

Once Opus returns its findings, use the Task tool with `model: "claude-sonnet-4-5"` (or the latest Sonnet) and `subagent_type: "general-purpose"`. Pass it the original code AND Opus's findings.

Sonnet's job is to:
1. Write a clean, well-organized review document grouped by severity (critical → high → medium → low → nits).
2. For each finding, produce a concrete code suggestion — actual diff-style before/after blocks where possible.
3. Flag any Opus findings that look incorrect or contradict the code (sanity check).
4. Add a "Quick wins" section listing fixes that take under 5 minutes.
5. End with a short "Production readiness verdict": ready / ready with caveats / not ready, with one-line reasoning.

Prompt:

> You are implementing a code review report. You have been given (1) the source code under review and (2) findings from a senior reviewer. Your job is to turn this into a polished review document and write concrete fix suggestions.
>
> Organize by severity. For each finding, show the problematic code and the suggested replacement as diff blocks. Sanity-check each finding against the actual code — if a finding is wrong, say so and skip it. Add a "Quick wins" section and a final "Production readiness" verdict.
>
> Format the output as Markdown. Be concrete, not generic.

## Step 4 — Present results

Save the final review to `code-review-<timestamp>.md` in the working directory and show the user a brief summary in chat:

- Total findings by severity
- The production readiness verdict
- The top 3 critical/high items inline
- A pointer to the full file

Then ask if they'd like you to apply any of the suggested fixes.

## Notes on model selection

- Always use Opus for the analysis pass — that's the whole point of this skill. If Opus isn't available in the environment, tell the user before falling back.
- Sonnet handles the writing/implementation pass because it's faster and equally good at structured output work.
- If the codebase is small (< 200 lines), you may skip the subagent split and do both passes inline, but still explicitly think hard during the analysis pass.

## What this skill is NOT

Not a security audit. If the user wants security/pentest analysis, point them at `/code-security` instead — that's a separate skill with a different methodology.
