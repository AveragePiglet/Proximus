import React, { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface LogEntry {
  timestamp: string;
  source: string;
  level: string;
  message: string;
}

const LEVEL_COLORS: Record<string, string> = {
  info: "var(--text-secondary)",
  warn: "var(--warning, #e0af68)",
  error: "var(--error, #f7768e)",
};

export const LogsPanel: React.FC = () => {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Load history on mount
  useEffect(() => {
    invoke<LogEntry[]>("get_log_history").then(setEntries).catch(() => {});
  }, []);

  // Listen for new entries
  useEffect(() => {
    const unlisten = listen<LogEntry>("log-entry", (event) => {
      setEntries((prev) => {
        const next = [...prev, event.payload];
        return next.length > 500 ? next.slice(-500) : next;
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 40);
  }, []);

  const formatTime = (ts: string) => {
    try {
      const d = new Date(ts);
      return d.toLocaleTimeString("en-GB", { hour12: false });
    } catch {
      return ts;
    }
  };

  return (
    <div className="side-panel-content logs-panel">
      <div className="sidebar-section-header">
        <h3>Logs</h3>
        <span className="logs-count">{entries.length}</span>
      </div>

      {/* Log entries */}
      <div className="logs-scroll" ref={scrollRef} onScroll={handleScroll}>
        {entries.length === 0 ? (
          <div className="sidebar-placeholder">
            <p>No log entries</p>
          </div>
        ) : (
          entries.map((entry, i) => (
            <div key={i} className="log-entry">
              <span className="log-time">{formatTime(entry.timestamp)}</span>
              <span
                className="log-message"
                style={{
                  color:
                    entry.level !== "info"
                      ? LEVEL_COLORS[entry.level]
                      : undefined,
                }}
              >
                {entry.message}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
