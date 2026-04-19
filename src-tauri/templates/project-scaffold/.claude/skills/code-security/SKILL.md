---
name: code-security
description: Perform an in-depth security review and pentest-style analysis of code using Opus for deep adversarial reasoning. Use this skill whenever the user runs `/code-security`, asks for a "security review", "security audit", "pentest", "vulnerability scan", "threat model", or wants to know if code is "production ready" from a security perspective. Trigger this even when the user just says things like "is this safe?", "check for vulnerabilities", or "look for security issues" — prefer this skill over a casual security pass because it produces a far more rigorous adversarial analysis covering OWASP Top 10, auth/authz, injection, secrets, supply chain, and abuse paths.
---

# Code Security Review (Opus adversarial analysis)

A deep, adversarial security review. Opus drives the entire analysis because security review benefits enormously from the strongest reasoning model — vulnerabilities hide in interactions between components, and missing one matters more than producing a slightly slower report.

## When this skill runs

The user has typed `/code-security` or asked for a security review, audit, pentest, or vulnerability check. Your job is to orchestrate a rigorous adversarial analysis using Opus and present actionable findings.

## Step 1 — Determine the scope and gather context

Before launching the analysis, gather everything Opus will need to think adversarially:

1. **Code under review** — files explicitly named, otherwise the current branch's diff vs main, otherwise the whole working tree. Run git commands as needed.
2. **Dependencies** — read `package.json` / `requirements.txt` / `go.mod` / `Cargo.toml` / `pom.xml` etc. Supply chain matters.
3. **Config & env** — `.env.example`, config files, Dockerfiles, CI workflows, IaC files. Look for what services are exposed and how.
4. **Entry points** — HTTP routes, CLI handlers, message queue consumers, webhooks, public APIs. Note them explicitly.
5. **Trust boundaries** — where untrusted input enters the system. This is the single most important thing to identify before the analysis.

Don't ask the user for any of this if you can find it yourself. Only ask if the codebase is genuinely opaque about deployment context (e.g., "is this internet-facing or internal-only?").

## Step 2 — Launch the Opus security analysis subagent

Use the Task tool with `model: "claude-opus-4-5"` (or the latest Opus available) and `subagent_type: "general-purpose"`. Pass it the gathered code, dependencies, config, and your notes on entry points and trust boundaries.

Use this prompt:

> You are a senior application security engineer performing an adversarial security review and pentest-style analysis. Think hard and think like an attacker. Your goal is to find every way this code could be exploited, abused, or made to fail in ways that compromise confidentiality, integrity, or availability.
>
> Analyze across these dimensions. For each, actively try to construct an exploit, not just spot a smell:
>
> 1. **Injection** — SQL, NoSQL, command, LDAP, XPath, template, header, log. Trace every place untrusted input reaches an interpreter.
> 2. **AuthN / AuthZ** — broken authentication, missing authorization checks, IDOR, privilege escalation, JWT issues (alg confusion, weak secrets, missing exp), session fixation, insecure password storage, missing rate limits on auth endpoints.
> 3. **Secrets & cryptography** — hardcoded secrets, secrets in logs, weak/custom crypto, ECB mode, missing IV randomness, weak hashing for passwords (MD5/SHA1/unsalted), insecure randomness for security purposes.
> 4. **Input validation & deserialization** — unsafe deserialization (pickle, yaml.load, Java serialization), XXE, SSRF, open redirect, path traversal, prototype pollution, mass assignment.
> 5. **OWASP Top 10 (current)** — walk it explicitly.
> 6. **Web-specific** — CSRF, XSS (reflected/stored/DOM), clickjacking, CORS misconfig, missing security headers, cookie flags (Secure, HttpOnly, SameSite).
> 7. **API security** — missing auth on endpoints, verbose error messages, lack of rate limiting, unbounded queries, mass data exposure, BOLA/BFLA.
> 8. **Supply chain** — known-vulnerable dependencies (call out specific CVEs if you recognize them), typosquats, unpinned versions, postinstall scripts, untrusted registries.
> 9. **Secrets management & config** — debug mode in production, default credentials, world-readable secrets, secrets in version control, overly permissive IAM/CORS/firewall.
> 10. **Race conditions & TOCTOU** — especially around auth, file operations, payment, and resource limits.
> 11. **Denial of service** — algorithmic complexity attacks (ReDoS, hash collision), unbounded resource consumption, zip bombs, billion laughs.
> 12. **Logging & monitoring** — sensitive data in logs, missing audit trail for security events, log injection.
> 13. **Business logic abuse** — workflows that can be skipped, replayed, reordered, or run with manipulated state. Think about what an attacker would *want* to do with this app and whether they can.
>
> For EACH finding return:
> - `severity`: critical / high / medium / low / informational (use CVSS-style reasoning)
> - `category`: which dimension above
> - `cwe`: relevant CWE ID if you know one
> - `location`: file:line
> - `vulnerability`: 1-2 sentence description
> - `exploit_scenario`: a concrete step-by-step attack — what the attacker sends, what happens, what they gain. Be specific. "An attacker could potentially..." is not acceptable. Show the actual request/payload/sequence.
> - `impact`: what's compromised and how bad
> - `remediation`: concrete fix with code-level guidance
> - `references`: OWASP/CWE/CVE links if relevant
>
> Also produce:
> - A **threat model summary**: trust boundaries, attacker profiles (unauthenticated remote, authenticated user, malicious insider, compromised dependency), and the highest-value targets in this code.
> - A **production readiness verdict**: SHIP / SHIP WITH FIXES / DO NOT SHIP, with one-paragraph reasoning grounded in the highest-severity findings.
>
> Be thorough. Missing a critical vulnerability is much worse than reporting a borderline one. If you're unsure whether something is exploitable, include it and mark it as "needs verification". Use extended thinking aggressively.

## Step 3 — Sanity-check and write the report

Once Opus returns findings, do a quick sanity pass yourself:

- Skim each critical/high finding against the actual code. If one is clearly wrong (e.g., references a function that doesn't exist, or the "vulnerable" code is actually inside a guarded branch), mark it as "Opus flagged this but I couldn't reproduce" rather than dropping it silently.
- Group findings by severity, then by category.
- For each finding, format the exploit scenario clearly — this is what the user actually needs to understand the risk.

Save the full report to `security-review-<timestamp>.md` in the working directory.

## Step 4 — Present results

In chat, give the user:

1. The **production readiness verdict** front and center.
2. A count of findings by severity.
3. The **top 3 critical/high vulnerabilities** with their exploit scenarios inline (not just titles — the user needs to grasp the actual risk without opening the file).
4. The threat model summary in 3-5 bullets.
5. A pointer to the full report file.

Then ask whether they'd like you to: (a) apply remediation fixes, (b) write regression tests that would catch these vulnerabilities, or (c) dig deeper on any specific finding.

## Notes on model selection

- This skill **always** uses Opus for the analysis. Security review is the highest-stakes use of model intelligence in code work — never downgrade to Sonnet for the analysis pass without explicitly telling the user first.
- If Opus is not available in the environment, tell the user upfront and let them decide whether to proceed with Sonnet or wait.
- The codebase being small is not a reason to skip Opus — small codebases can still hide critical vulnerabilities, and the cost is negligible.

## What this skill is NOT

Not a general code review. If the user wants correctness, design, performance, or maintainability feedback, point them at `/code-review` instead.

Not a substitute for a real penetration test against a running system. This is static analysis with adversarial reasoning. Make this clear in the report — runtime issues (misconfigured infrastructure, exposed services, network-level issues) need a real pentest.
