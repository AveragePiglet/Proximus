import React, { useState, useEffect } from "react";
import { useTabStatus } from "../hooks/useTabStatus";

interface StatusBarProps {
  tabId: string;
}

function formatDuration(startIso: string): string {
  const start = new Date(startIso).getTime();
  const now = Date.now();
  const diffSec = Math.floor((now - start) / 1000);
  if (diffSec < 60) return `${diffSec}s`;
  const mins = Math.floor(diffSec / 60);
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  const remMins = mins % 60;
  return `${hrs}h ${remMins}m`;
}

function formatAgo(isoStr: string): string {
  const ts = new Date(isoStr).getTime();
  const now = Date.now();
  const diffSec = Math.floor((now - ts) / 1000);
  if (diffSec < 5) return "Just now";
  if (diffSec < 60) return `${diffSec}s ago`;
  const mins = Math.floor(diffSec / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ago`;
}

function contextColor(pct: number): string {
  if (pct < 50) return "low";
  if (pct < 80) return "mid";
  return "high";
}

export const StatusBar: React.FC<StatusBarProps> = ({ tabId }) => {
  const status = useTabStatus(tabId);
  const [, setTick] = useState(0);

  // Re-render every 15s so duration/ago stay fresh
  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 15_000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="status-bar">
      <div className="status-bar-item">
        <span className={`status-bar-dot ${status.pty_running ? "running" : "stopped"}`} />
        <span className="status-bar-label">{status.pty_running ? "Running" : "Stopped"}</span>
      </div>
      {status.pty_started_at && (
        <div className="status-bar-item">
          <span className="status-bar-label">⏱ {formatDuration(status.pty_started_at)}</span>
        </div>
      )}
      <div className="status-bar-item">
        <span className="status-bar-label">
          💾 {status.last_memory_save ? formatAgo(status.last_memory_save) : "No saves yet"}
        </span>
      </div>
      <div className="status-bar-item">
        <div className="status-bar-context-bar">
          <div
            className={`status-bar-context-fill ${contextColor(status.context_percent)}`}
            style={{ width: `${Math.min(status.context_percent, 100)}%` }}
          />
        </div>
        <span className="status-bar-label">
          {status.context_percent.toFixed(0)}% context
          ({(status.tokens_used / 1000).toFixed(1)}K / {(status.tokens_total / 1000).toFixed(1)}K)
        </span>
      </div>
      {status.cost_usd > 0 && (
        <div className="status-bar-item">
          <span className="status-bar-label">💰 ${status.cost_usd.toFixed(2)}</span>
        </div>
      )}
    </div>
  );
};
