---
name: ux-review
description: Perform an in-depth UX/UI and user experience review of a web or app frontend, checking visual consistency, usability, accessibility, and parity between desktop and mobile views. Use this skill whenever the user runs `/ux-review`, asks for a "UX review", "UI review", "design review", "usability check", "accessibility check", "responsive review", or wants to compare desktop vs mobile behavior. Trigger this even when the user just says "review my UI" or "check if this looks good on mobile" — prefer this skill over a casual visual pass because it produces a rigorous review covering layout, typography, spacing, color, interaction patterns, accessibility, and cross-viewport parity.
---

# UX / UI Review

A deep frontend review covering visual consistency, usability, accessibility, and desktop-vs-mobile parity. Uses Opus for the analysis pass because UX review benefits from holistic reasoning across many interacting concerns — a button that's fine in isolation can be wrong in context.

## When this skill runs

The user has typed `/ux-review` or asked for a UX/UI/design/usability/responsive review. Your job is to gather the frontend under review at both viewports, run a rigorous analysis, and present actionable findings.

## Step 1 — Determine what to review and how to capture it

Figure out the target. In order of preference:

1. A running local dev server (look for `npm run dev`, `vite`, `next dev`, etc. in `package.json` — the user may already have it running on a known port).
2. A deployed URL the user provides.
3. Static HTML/CSS/component files in the repo.
4. Design files or screenshots the user has uploaded.

Ask the user only if it's genuinely unclear. If a dev server isn't running and the user wants a live review, offer to start it for them.

### Routes / screens to capture

Identify the key screens. Don't review only the landing page. Look for: routes in the router config, page files in `pages/` or `app/`, top nav links. Pick the 4-8 most important screens (landing, primary auth flow, main app surface, settings, any high-stakes flows like checkout). If the user named specific screens, use those.

### Capturing both viewports

This is the core of the skill — every screen gets captured at **both** desktop and mobile widths. Use Playwright (preferred) or Puppeteer. If neither is installed, install Playwright:

```bash
npm install -D @playwright/test && npx playwright install chromium
```

Then for each screen, run a script that captures full-page screenshots at both widths. Use these viewports:

- **Desktop**: 1440 × 900 (standard laptop)
- **Mobile**: 390 × 844 (iPhone 14)

Save screenshots to `ux-review-<timestamp>/screenshots/<screen>-{desktop,mobile}.png`. Also capture the rendered DOM (`page.content()`) and the computed accessibility tree (`page.accessibility.snapshot()`) for each viewport — these are essential for the analysis pass.

For interactive elements (dropdowns, modals, mobile nav drawer), capture the open state too, not just the closed state. A mobile hamburger menu that doesn't open is invisible to a screenshot-only review.

### Use the read tool on screenshots

After capturing, use the `Read` tool on each screenshot file so you can actually see the rendered UI. Don't try to review from DOM alone — visual issues need visual inspection.

## Step 2 — Launch the Opus UX analysis

Use the Task tool with `model: "claude-opus-4-5"` (or the latest Opus available) and `subagent_type: "general-purpose"`. Pass it the screenshots, DOM snapshots, accessibility trees, and any relevant component source code.

Use this prompt:

> You are a senior product designer and frontend engineer performing a deep UX/UI review. Think hard. You have screenshots of each screen at desktop (1440×900) and mobile (390×844), plus DOM and accessibility trees. Analyze across these dimensions:
>
> 1. **Visual consistency** — typography scale, font weights, color palette, spacing scale, border radii, shadows, icon style. Are tokens used consistently or are there one-off values? Do buttons across screens look like the same button?
> 2. **Layout & hierarchy** — is the visual hierarchy clear? Does the eye know where to go first? Are primary actions distinguishable from secondary? Is whitespace doing work or is the layout cramped/sparse?
> 3. **Typography** — readable line lengths (45-75ch for body), sufficient line height (1.4-1.6 for body), clear heading hierarchy, no orphaned headings, no walls of text without rhythm.
> 4. **Color & contrast** — WCAG AA minimum (4.5:1 for body text, 3:1 for large text and UI components). Check actual computed colors, not vibes. Flag any failing pairs.
> 5. **Interaction affordances** — do buttons look clickable? Do links look like links? Are disabled states obviously disabled? Are hover/focus/active states defined?
> 6. **Forms** — labels (not just placeholders), clear required indicators, inline validation, sensible input types, error messages that explain how to fix the problem, logical tab order.
> 7. **Accessibility** — semantic HTML, proper heading order (no h1 → h3 jumps), alt text on meaningful images, ARIA only where needed and used correctly, focus indicators visible, keyboard reachable, sufficient touch targets (44×44 minimum on mobile).
> 8. **Mobile-specific** — touch targets, thumb reach, no hover-dependent functionality, no horizontal scroll, readable without zoom, fixed elements don't eat the viewport, safe-area insets respected.
> 9. **Empty / loading / error states** — does each major surface have all four states designed (empty, loading, error, success)? Or only the happy path?
> 10. **Microcopy** — clear, concise, consistent voice, no jargon, no "Click here", error messages that help rather than blame.
>
> 11. **Desktop ↔ mobile parity** (THIS IS CRITICAL — give it special attention):
>     - **Feature parity**: is every action available on desktop also available on mobile, even if reorganized? Flag anything that exists on one but not the other.
>     - **Logic parity**: do flows behave the same way? Same validation, same error handling, same outcomes?
>     - **Content parity**: is information truncated or hidden on mobile in ways that lose meaning? Are tables turned into something usable, or just clipped?
>     - **Visual coherence**: does the mobile version feel like the same product as desktop? Same brand, same patterns adapted appropriately — not a different app.
>     - **Navigation parity**: can users reach every screen on both? Is the mobile nav (drawer/tabs) complete?
>     - **Interaction parity**: are hover-only interactions given a tap equivalent on mobile? Are right-click menus reachable? Are drag interactions usable on touch?
>
> For EACH finding return:
> - `severity`: critical / high / medium / low / nit
> - `category`: which dimension above
> - `viewport`: desktop / mobile / both
> - `screen`: which screen(s) this affects
> - `issue`: 1-2 sentence description
> - `why_it_matters`: user impact in 1-2 sentences — what does the user actually experience?
> - `recommendation`: concrete fix, with specific values where possible (e.g., "increase touch target to 44×44", "use design token `--space-4` instead of hardcoded 18px")
>
> Also produce:
> - A **consistency audit**: list inconsistencies you found across screens (e.g., "primary buttons use 6px radius on /login but 8px on /signup", "h2 is 24px on dashboard but 28px on settings")
> - A **parity diff**: a concrete table/list of things that differ between desktop and mobile in ways that aren't intentional responsive adaptation
> - An **overall UX verdict**: ship-ready / needs polish / needs significant work, with one paragraph reasoning
>
> Be specific. "Spacing feels off" is not acceptable — say which element, which screen, what value it has, and what it should be. Use the screenshots, don't guess from the DOM.

## Step 3 — Sanity check and write the report

When Opus returns findings, do a quick pass yourself against the screenshots:

- For any critical/high finding, glance at the relevant screenshot and confirm it's real. Mark anything you can't reproduce as "needs verification".
- Group findings by severity, then by screen.
- Pull out the parity diff into its own prominent section — that's what the user specifically cares about.

Save the full report to `ux-review-<timestamp>/report.md` alongside the screenshots so the user can cross-reference visuals against findings.

## Step 4 — Present results

In chat, give the user:

1. The **overall UX verdict** front and center.
2. Counts of findings by severity.
3. The **parity diff** (desktop vs mobile differences) — this is the headline item.
4. The **top 3-5 critical/high issues** with the screen and viewport noted, and a one-line description each.
5. A pointer to the report file and the screenshots directory.

Then ask whether they'd like you to: (a) implement specific fixes, (b) generate a design tokens file from the inconsistencies found, or (c) dig deeper on any screen.

## Notes

- **Always capture both viewports.** Skipping mobile defeats the purpose of this skill. If you genuinely can't (e.g., the user only gave you desktop screenshots), tell them upfront and ask for mobile views before proceeding.
- **Use Opus for the analysis.** UX review involves reasoning across many interacting concerns simultaneously — this is exactly where the strongest model pays off. If Opus isn't available, tell the user before falling back to Sonnet.
- **Read the screenshots yourself, not just pass them to a subagent.** You need to see the UI to do the sanity-check pass in step 3.

## What this skill is NOT

Not a code review. If the user wants the underlying React/Vue/Svelte code reviewed for correctness or structure, point them at `/code-review`.

Not a security review. If the user wants frontend security analysis (XSS, CSP, etc.), point them at `/code-security`.

Not a substitute for real user testing. Make this clear in the report — heuristic review catches a lot, but only real users catch certain classes of issues.
