import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TerminalHandle } from "./Terminal";

interface QuickActionsProps {
  tabId: string;
  terminalRef?: React.RefObject<TerminalHandle | null>;
  onRecoveringChange?: (recovering: boolean) => void;
  tabType?: string;
}

const ACTIONS = [
  { label: "Update Memory", command: "Update Memory", projectOnly: true },
  { label: "Load Memory", command: "Load Memory", projectOnly: true },
  { label: "Compact", command: "/compact", projectOnly: false },
  { label: "Clear", command: "/clear", projectOnly: false },
];

export const QuickActions: React.FC<QuickActionsProps> = ({ tabId, terminalRef, onRecoveringChange, tabType }) => {
  const [recovering, setRecovering] = useState(false);
  const isProject = tabType === "project";
  const visibleActions = ACTIONS.filter(a => !a.projectOnly || isProject);

  const run = (command: string) => {
    invoke("write_pty", { tabId, data: command + "\r" }).catch(() => {});
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
      <span className="quick-actions-label">Actions</span>
      {visibleActions.map((a) => (
        <button key={a.command} className="quick-action-btn" onClick={() => run(a.command)} disabled={recovering}>
          {a.label}
        </button>
      ))}
      <button className="quick-action-btn recover" onClick={recover} disabled={recovering}>
        {recovering ? "Recovering..." : "Recover"}
      </button>
    </div>
  );
};
