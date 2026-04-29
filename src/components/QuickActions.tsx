import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TerminalHandle } from "./Terminal";

interface QuickActionsProps {
  tabId: string;
  terminalRef?: React.RefObject<TerminalHandle | null>;
  onRecoveringChange?: (recovering: boolean) => void;
  tabType?: string;
  projectPath?: string;
}

interface ValidateResult {
  passed: boolean;
  output: string;
}

const ACTIONS = [
  { label: "Update Memory", command: "Update Memory", projectOnly: true },
  { label: "Load Memory", command: "Load Memory", projectOnly: true },
  { label: "Compact", command: "/compact", projectOnly: false },
  { label: "Clear", command: "/clear", projectOnly: false },
];

export const QuickActions: React.FC<QuickActionsProps> = ({ tabId, terminalRef, onRecoveringChange, tabType, projectPath }) => {
  const [recovering, setRecovering] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validateResult, setValidateResult] = useState<ValidateResult | null>(null);
  const isProject = tabType === "project";
  const visibleActions = ACTIONS.filter(a => !a.projectOnly || isProject);

  const run = (command: string) => {
    invoke("write_pty", { tabId, data: command + "\r" }).catch(() => {});
  };

  const validateMemory = async () => {
    setValidating(true);
    setValidateResult(null);
    try {
      const result = await invoke<ValidateResult>("validate_memory", { tabId });
      setValidateResult(result);

      const validatorStatus = result.passed
        ? `validate.py PASSED: ${result.output}`
        : `validate.py FAILED:\n${result.output}`;

      const prompt = `Validate Memory — run a full memory health check now.

Validator result: ${validatorStatus}

Check each of the following and act on anything that needs fixing:
1. state.toml — clear active_task if no task is running; verify next_action is still accurate; remove any known_issues that are confirmed fixed
2. graph.toml — bump last_touched on any nodes you worked in this session; verify L0/L1 summaries are still accurate
3. nodes/*.toml — update any L2 content that is stale or missing recent work
4. journal/ — ensure today's session has an entry; append one if missing
5. invariants.toml — add any new hard rules discovered this session
6. plans/ — if state.toml references an active_plan, verify it is up to date; if a plan was completed, mark it done; check no plan files exist in the project root (⊥ loose plans outside .node-memory/plans/)
7. Hard rules audit — verify none of these are violated:
   - ⊥ prose paragraphs in memory files (lists and structured TOML only)
   - ⊥ duplicating facts across files (reference by ID instead)
   - ⊥ nodes without at least one edge
   - ⊥ plan files outside .node-memory/plans/
   - ⊤ node files ≤ 120 lines (fix any that exceed this)
   - ⊤ every bug, invariant, decision, task has a stable ID
8. If validate.py failed — fix the specific errors reported above before anything else

After completing all checks, run validate.py again (python .node-memory/tools/validate.py) and confirm it passes.`;

      run(prompt);
    } catch (e) {
      setValidateResult({ passed: false, output: `Error: ${e}` });
    } finally {
      setValidating(false);
      setTimeout(() => setValidateResult(null), 6000);
    }
  };

  const recover = async () => {
    setRecovering(true);
    onRecoveringChange?.(true);

    // Grab terminal buffer BEFORE clearing — this is the errored conversation
    const bufferText = terminalRef?.current?.getBufferText(200) ?? "";
    const lines = bufferText.split("\n");
    // Filter out empty lines and Claude UI chrome, keep meaningful content
    const contextLines = lines
      .map(l => l.trim())
      .filter(l => l.length > 0)
      .filter(l => !l.startsWith("─") && !l.startsWith("│") && !l.startsWith("┌") && !l.startsWith("└") && !l.startsWith("┐") && !l.startsWith("┘"))
      .filter(l => !l.includes("Welcome back") && !l.includes("Tips for getting started") && !l.includes("Recent activity") && !l.includes("Claude Code v") && !l.includes("/clear") && !l.includes("(no content)") && !l.includes("Previous session errored") && !l.includes("Recovering session"))
      .join(" | ")
      .slice(0, 3000);

    // Store the context before we clear
    const savedContext = contextLines;

    // Clear context
    run("/clear");

    await new Promise((r) => setTimeout(r, 3000));

    // Send resume command with full captured context, then confirm the paste
    const msg = "=== Previous Session Output === " + savedContext + " === End of Previous Session === [RECOVERY] The previous Claude session crashed/errored and had to be cleared. The text above is the captured terminal output from before the crash. Please read through it, acknowledge what was happening, and continue seamlessly from where things left off.";
    invoke("write_pty", { tabId, data: msg }).catch(() => {});
    // Wait for paste detection, then send Enter to confirm
    await new Promise((r) => setTimeout(r, 800));
    invoke("write_pty", { tabId, data: "\r" }).catch(() => {});

    // Keep locked briefly while Claude starts processing
    await new Promise((r) => setTimeout(r, 1000));

    setRecovering(false);
    onRecoveringChange?.(false);
  };

  return (
    <div className="quick-actions">
      {/* Left group — workflow actions */}
      <div className="quick-actions-group">
        {visibleActions.map((a) => (
          <button key={a.command} className="quick-action-btn" onMouseDown={(e) => e.preventDefault()} onClick={() => run(a.command)} disabled={recovering}>
            {a.label}
          </button>
        ))}
        {isProject && (
          <button
            className={`quick-action-btn validate ${validateResult ? (validateResult.passed ? "validate-pass" : "validate-fail") : ""}`}
            onMouseDown={(e) => e.preventDefault()}
            onClick={validateMemory}
            disabled={recovering || validating}
            title={validateResult?.output ?? "Run validate.py and ask AI to review outstanding memory items"}
          >
            {validating ? "Validating…" : validateResult ? (validateResult.passed ? "✓ Valid" : "✗ Invalid") : "Validate Memory"}
          </button>
        )}
      </div>

      <div className="quick-actions-spacer" />

      {/* Right group — folder + danger */}
      <div className="quick-actions-divider" />
      <div className="quick-actions-group">
        {isProject && projectPath && (
          <button
            className="quick-action-btn open-folder"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => invoke("open_project_folder", { path: projectPath }).catch(() => {})}
            title="Open project folder in file explorer"
          >
            Open Folder
          </button>
        )}
        <button className="quick-action-btn recover" onMouseDown={(e) => e.preventDefault()} onClick={recover} disabled={recovering}>
          {recovering ? "Recovering…" : "⟳ Recover"}
        </button>
      </div>
    </div>
  );
};
