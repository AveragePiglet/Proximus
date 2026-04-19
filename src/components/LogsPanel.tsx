import React, { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface LogEntry {
  timestamp: string;
  source: string;
  level: string;
  message: string;
}

const SOURCES = ["proxy", "app", "pty", "memory"];
const LEVELS = ["info", "warn", "error"];

const LEVEL_COLORS: Record<string, string> = {
  info: "#7aa2f7",
  warn: "#e0af68",
  error: "#f7768e",
};

const SOURCE_COLORS: Record<string, string> = {
  proxy: "#bb9af7",
  app: "#7aa2f7",
  pty: "#9ece6a",
  memory: "#e0af68",
};

export const LogsPanel: React.FC = () => {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [sourceFilter, setSourceFilter] = useState<Set<string>>(new Set(SOURCES));
  const [levelFilter, setLevelFilter] = useState<Set<string>>(new Set(LEVELS));
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

  const toggleSource = (src: string) => {
    setSourceFilter((prev) => {
      const next = new Set(prev);
      next.has(src) ? next.delete(src) : next.add(src);
      return next;
    });
  };

  const toggleLevel = (lvl: string) => {
    setLevelFilter((prev) => {
      const next = new Set(prev);
      next.has(lvl) ? next.delete(lvl) : next.add(lvl);
      return next;
    });
  };

  const filtered = entries.filter(
    (e) => sourceFilter.has(e.source) && levelFilter.has(e.level)
  );

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
        <span className="logs-count">{filtered.length}</span>
      </div>

      {/* Filter chips */}
      <div className="logs-filters">
        <div className="logs-filter-row">
          {SOURCES.map((src) => (
            <button
              key={src}
              className={`logs-chip${sourceFilter.has(src) ? " active" : ""}`}
              style={{
                borderColor: sourceFilter.has(src)
                  ? SOURCE_COLORS[src]
                  : undefined,
              }}
              onClick={() => toggleSource(src)}
            >
              {src}
            </button>
          ))}
        </div>
        <div className="logs-filter-row">
          {LEVELS.map((lvl) => (
            <button
              key={lvl}
              className={`logs-chip${levelFilter.has(lvl) ? " active" : ""}`}
              style={{
                borderColor: levelFilter.has(lvl)
                  ? LEVEL_COLORS[lvl]
                  : undefined,
              }}
              onClick={() => toggleLevel(lvl)}
            >
              {lvl}
            </button>
          ))}
        </div>
      </div>

      {/* Log entries */}
      <div className="logs-scroll" ref={scrollRef} onScroll={handleScroll}>
        {filtered.length === 0 ? (
          <div className="sidebar-placeholder">
            <p>No log entries</p>
          </div>
        ) : (
          filtered.map((entry, i) => (
            <div key={i} className={`log-entry log-level-${entry.level}`}>
              <span className="log-time">{formatTime(entry.timestamp)}</span>
              <span
                className="log-source"
                style={{ color: SOURCE_COLORS[entry.source] || "#a9b1d6" }}
              >
                {entry.source}
              </span>
              <span className="log-message">{entry.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
