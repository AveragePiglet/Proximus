import { useEffect, useRef, useImperativeHandle, forwardRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";

interface TerminalProps {
  tabId: string;
  locked?: boolean;
}

interface PtyOutputEvent {
  tab_id: string;
  data: string;
}

export interface TerminalHandle {
  getBufferText: (maxLines?: number) => string;
}

export const Terminal = forwardRef<TerminalHandle, TerminalProps>(({ tabId, locked }, ref) => {
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);

  useImperativeHandle(ref, () => ({
    getBufferText: (maxLines = 200) => {
      const term = xtermRef.current;
      if (!term) return "";
      const buf = term.buffer.active;
      const totalLines = buf.length;
      const start = Math.max(0, totalLines - maxLines);
      const lines: string[] = [];
      for (let i = start; i < totalLines; i++) {
        const line = buf.getLine(i);
        if (line) lines.push(line.translateToString(true));
      }
      // Trim trailing empty lines
      while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
        lines.pop();
      }
      return lines.join("\n");
    },
  }));

  useEffect(() => {
    if (!termRef.current) return;

    const term = new XTerm({
      theme: {
        background: "#1a1b26",
        foreground: "#a9b1d6",
        cursor: "#c0caf5",
        selectionBackground: "#33467c",
        black: "#15161e",
        red: "#f7768e",
        green: "#9ece6a",
        yellow: "#e0af68",
        blue: "#7aa2f7",
        magenta: "#bb9af7",
        cyan: "#7dcfff",
        white: "#a9b1d6",
      },
      fontSize: 14,
      fontFamily: "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
      cursorBlink: true,
      cursorStyle: "bar",
      scrollback: 5000,
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(termRef.current);
    fitAddon.fit();
    xtermRef.current = term;

    // Undo buffer: groups of characters chunked by typing pauses
    // Each entry is a group of chars typed within UNDO_GROUP_MS of each other
    const UNDO_GROUP_MS = 600;
    const undoGroups: string[][] = []; // each group is an array of chars
    let lastInputTime = 0;

    const pushToUndo = (chars: string[]) => {
      const now = Date.now();
      if (undoGroups.length === 0 || now - lastInputTime > UNDO_GROUP_MS) {
        // Start a new group
        undoGroups.push([...chars]);
      } else {
        // Append to current group
        undoGroups[undoGroups.length - 1].push(...chars);
      }
      lastInputTime = now;
    };

    const popFromUndo = () => {
      // Remove the last character from the latest group
      if (undoGroups.length === 0) return;
      undoGroups[undoGroups.length - 1].pop();
      if (undoGroups[undoGroups.length - 1].length === 0) {
        undoGroups.pop();
      }
    };

    // Intercept Ctrl+C (copy), Ctrl+V (paste), Ctrl+Z (undo)
    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;
      if (event.ctrlKey && event.key === "c" && term.hasSelection()) {
        navigator.clipboard.writeText(term.getSelection());
        term.clearSelection();
        return false;
      }
      if (event.ctrlKey && event.key === "v") {
        // preventDefault stops the browser from also firing a paste event
        event.preventDefault();
        navigator.clipboard.readText().then((text) => {
          // Paste is always its own undo group
          undoGroups.push([...text]);
          lastInputTime = Date.now();
          invoke("write_pty", { tabId, data: text }).catch(() => {});
        });
        return false;
      }
      if (event.ctrlKey && event.key === "z") {
        event.preventDefault();
        if (undoGroups.length > 0) {
          const group = undoGroups.pop()!;
          // Send one backspace per character in the group
          const backspaces = "\x7f".repeat(group.length);
          invoke("write_pty", { tabId, data: backspaces }).catch(() => {});
        }
        return false;
      }
      return true;
    });

    // Send keystrokes to this tab's PTY
    term.onData((data) => {
      // Track input for undo buffer
      if (data === "\r" || data === "\n") {
        // Enter pressed — clear undo history
        undoGroups.length = 0;
      } else if (data === "\x7f" || data === "\x08") {
        // Backspace — pop one char from undo to stay in sync
        popFromUndo();
      } else if (data.length === 1 && data >= " ") {
        // Regular printable character
        pushToUndo([data]);
      }
      invoke("write_pty", { tabId, data }).catch(() => {});
    });

    // Receive PTY output filtered by tab_id
    let ptyMessageCount = 0;
    const unlistenPty = listen<PtyOutputEvent>("pty-output", (event) => {
      if (event.payload.tab_id !== tabId) return;
      ptyMessageCount++;
      if (ptyMessageCount === 1) {
        term.clear();
      }
      term.write(event.payload.data);
    });

    // Handle resize
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      invoke("resize_pty_cmd", { tabId, rows: term.rows, cols: term.cols }).catch(() => {});
    });
    resizeObserver.observe(termRef.current);

    // Show a welcome message
    term.writeln("\x1b[1;36mProximus Workspace\x1b[0m");
    term.writeln('Launching Claude Code...');
    term.writeln("");

    return () => {
      unlistenPty.then((fn) => fn());
      resizeObserver.disconnect();
      term.dispose();
    };
  }, [tabId]);

  return (
    <div style={{ position: "relative", flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
      <div ref={termRef} className="terminal-container" />
      {locked && (
        <div className="terminal-lock-overlay">
          <span>Recovering session...</span>
        </div>
      )}
    </div>
  );
});
