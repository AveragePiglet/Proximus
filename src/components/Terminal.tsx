import { useEffect, useRef, useImperativeHandle, forwardRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getTheme, getSavedTheme, onThemeChange } from "../themes";
import "@xterm/xterm/css/xterm.css";

interface TerminalProps {
  tabId: string;
  locked?: boolean;
  projectName?: string;
}

interface PtyOutputEvent {
  tab_id: string;
  data: string;
}

export interface TerminalHandle {
  getBufferText: (maxLines?: number) => string;
}

export const Terminal = forwardRef<TerminalHandle, TerminalProps>(({ tabId, locked, projectName }, ref) => {
  const termRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const [ptyStarted, setPtyStarted] = useState(false);

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

    const currentTheme = getTheme(getSavedTheme());
    const term = new XTerm({
      theme: currentTheme?.terminal ?? {
        background: "#1a1b26",
        foreground: "#a9b1d6",
        cursor: "transparent",
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
      cursorBlink: false,
      cursorStyle: "bar",
      cursorInactiveStyle: "none",
      scrollback: 5000,
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(termRef.current);
    fitAddon.fit();
    xtermRef.current = term;

    // Intercept Ctrl+C (copy), Ctrl+V (paste)
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
          invoke("write_pty", { tabId, data: text, bracketedPaste: true }).catch(() => {});
        });
        return false;
      }
      return true;
    });

    // Send keystrokes to this tab's PTY
    term.onData((data) => {
      invoke("write_pty", { tabId, data }).catch(() => {});
    });

    // Receive PTY output filtered by tab_id
    let ptyMessageCount = 0;
    const unlistenPty = listen<PtyOutputEvent>("pty-output", (event) => {
      if (event.payload.tab_id !== tabId) return;
      ptyMessageCount++;
      if (ptyMessageCount === 1) {
        term.clear();
        setPtyStarted(true);
      }
      term.write(event.payload.data);
    });

    // Handle resize — coalesce via requestAnimationFrame so drag-resizing doesn't
    // flood the IPC channel with hundreds of resize_pty_cmd calls per second.
    let rafHandle: number | null = null;
    const resizeObserver = new ResizeObserver(() => {
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      rafHandle = requestAnimationFrame(() => {
        rafHandle = null;
        fitAddon.fit();
        invoke("resize_pty_cmd", { tabId, rows: term.rows, cols: term.cols }).catch(() => {});
      });
    });
    resizeObserver.observe(termRef.current);

    // Show a welcome message
    term.writeln("\x1b[1;36mProximus Workspace\x1b[0m");
    term.writeln('Launching Claude Code...');
    term.writeln("");

    // Listen for live theme changes
    const unsubTheme = onThemeChange((theme) => {
      term.options.theme = { ...theme.terminal, cursor: "transparent" };
    });

    return () => {
      unsubTheme();
      unlistenPty.then((fn) => fn());
      resizeObserver.disconnect();
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      term.dispose();
    };
  }, [tabId]);

  // Reset "started" state when tab changes so overlay shows again
  useEffect(() => { setPtyStarted(false); }, [tabId]);

  return (
    <div style={{ position: "relative", flex: 1, display: "flex", flexDirection: "column", overflow: "hidden", clipPath: "inset(0 0 14px 0)" }}>
      <div ref={termRef} className="terminal-container" />
      {!ptyStarted && !locked && (
        <div className="terminal-starting-overlay">
          <div className="terminal-starting-content">
            <span className="terminal-starting-icon">⚡</span>
            <span className="terminal-starting-name">{projectName || "Session"}</span>
            <span className="terminal-starting-dots">
              <span />
              <span />
              <span />
            </span>
            <span className="terminal-starting-label">Starting Claude Code…</span>
          </div>
        </div>
      )}
      {locked && (
        <div className="terminal-lock-overlay">
          <span>Recovering session...</span>
        </div>
      )}
    </div>
  );
});
