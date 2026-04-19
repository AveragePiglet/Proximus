---
name: commit
description: Generate a Conventional Commits-style title and a concise bulleted body summarizing everything changed since the last commit, show it to the user, and wait for confirmation before actually committing. Use this skill whenever the user runs `/commit`, says "commit my changes", "write a commit message", "stage and commit", or any similar phrasing. Trigger this even when the user just says "commit" — prefer this skill over running `git commit` directly because it produces a clean, reviewed commit message and never commits without explicit user approval.
---

# Commit (review-then-commit workflow)

Generate a high-quality commit message from the current uncommitted changes, present it for approval, and only commit after the user says yes.

## When this skill runs

The user has typed `/commit` or asked you to commit their work. Your job is to summarize what changed, propose a commit, get approval, and execute.

## Step 1 — Gather what changed

Run these in order to build a complete picture of the working state:

```bash
git status --short
git diff --stat
git diff            # unstaged changes
git diff --cached   # already-staged changes (if any)
```

If there are no changes at all, tell the user "nothing to commit" and stop.

If there are both staged and unstaged changes, note this — you'll mention it when presenting the message so the user knows what will actually be committed.

For large diffs, also peek at the full diff content for the most-changed files so the message reflects the *intent* of the change, not just the file list. Don't summarize from filenames alone — read the actual code.

## Step 2 — Write the commit message

Use **Conventional Commits** format. The title line is:

```
<type>(<scope>): <short description>
```

Where:
- **type** is one of: `feat`, `fix`, `refactor`, `perf`, `docs`, `style`, `test`, `build`, `ci`, `chore`, `revert`
- **scope** is optional — use it when changes are localized to a clear module/area
- **description** is imperative mood, lowercase, no trailing period, under 72 chars

Then a blank line, then a body of 2-6 concise bullet points covering what was implemented. Each bullet should be one line, start with a capital letter, and describe a meaningful change — not a file-by-file changelog. Group related edits.

**Good example:**
```
feat(auth): add refresh token rotation

- Issue new refresh token on every use and revoke the previous one
- Store token family ID to detect replay attacks across rotations
- Add 30-day absolute expiry independent of rotation cadence
- Update integration tests to cover rotation and replay paths
```

**Bad example (don't do this):**
```
Updated stuff

- Changed auth.py
- Modified tests
- Fixed bug
```

### Choosing the type

If the diff contains multiple kinds of changes, pick the type that reflects the *primary* intent. If it's genuinely mixed (e.g., a feature + unrelated refactor), suggest splitting into two commits before proceeding — but defer to the user if they say "just commit it all".

### Scope guidance

Look at the file paths. If everything lives under one directory (`src/auth/`, `api/users/`, `components/checkout/`), use that as the scope. If changes span the codebase, omit the scope rather than inventing a vague one.

## Step 3 — Present the message for approval

Show the user:

1. A quick summary of what's being committed: number of files, staged vs unstaged status, and whether anything is being left out.
2. The proposed commit message in a code block, exactly as it will be committed.
3. A direct question: "Commit this? (yes / edit / cancel)"

If there are unstaged changes, explicitly ask whether to `git add -A` first or only commit what's already staged. Don't assume.

**Do not run `git commit` yet.** Wait for the user's response.

## Step 4 — Handle the response

- **Yes / approve / go / ship it** → run `git add -A` (if agreed in step 3) and then `git commit -m "<title>" -m "<body>"`. Show the resulting commit hash and one-line confirmation.
- **Edit / change X / make it shorter / etc.** → revise the message based on the feedback and go back to Step 3. Don't commit until they explicitly approve a version.
- **Cancel / no / nevermind** → don't commit. Confirm you've left the working tree untouched.

## Important rules

- **Never commit without explicit approval.** The whole point of this skill is the review step. Even if the user seems impatient, always show the message first.
- **Never amend, force-push, or rewrite history** unless the user specifically asks for it. This skill creates new commits only.
- **Never add Claude/AI attribution** to commit messages (no "Generated with Claude", no co-author trailers) unless the user explicitly asks for it.
- **Don't invent a scope** or pad bullets to hit a count. A 2-bullet body is fine if that's what the change deserves.
- **If pre-commit hooks fail**, show the user the failure and ask whether to fix and retry or abort. Don't bypass hooks with `--no-verify` unless asked.

## What this skill is NOT

Not a code review. If the user wants their changes reviewed before committing, point them at `/code-review` first. This skill assumes the code is already where the user wants it and just needs a clean commit.
