import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { StatusBadge } from "./StatusBadge";
import { useProcessStatus } from "../hooks/useProcessStatus";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const Toolbar: React.FC = () => {
  const statuses = useProcessStatus();
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const hasAutoStarted = useRef(false);

  const startProxyChain = async () => {
    setError(null);
    setStarting(true);

    let proxyPort: number;
    try {
      setStatus("Starting copilot proxy...");
      proxyPort = await invoke<number>("start_copilot_proxy");
    } catch (e) {
      setError(`Copilot proxy failed: ${e}`);
      setStarting(false);
      setStatus(null);
      return;
    }

    setStatus(`Waiting for proxy on :${proxyPort}...`);
    await sleep(4000);

    try {
      setStatus("Starting model rewriter...");
      await invoke<number>("start_model_rewriter", { upstreamPort: proxyPort });
    } catch (e) {
      setError(`Model rewriter failed: ${e}`);
      setStarting(false);
      setStatus(null);
      return;
    }

    setStatus("Spawning PTYs...");
    try {
      const tabs = await invoke<Array<{ id: string; status: string }>>("get_tabs");
      for (const tab of tabs) {
        if (tab.status === "active") {
          await invoke("spawn_tab_pty", { tabId: tab.id }).catch((e: unknown) =>
            console.warn(`Failed to spawn PTY for tab ${tab.id}:`, e)
          );
        }
      }
    } catch (e) {
      console.warn("Failed to spawn tab PTYs:", e);
    }

    setStatus(null);
    setStarting(false);
  };

  const handleRestart = async () => {
    setError(null);
    setStatus("Restarting...");
    try {
      await invoke("stop_services");
    } catch (_) {}
    await sleep(1000);
    await startProxyChain();
  };

  // Auto-start on mount
  useEffect(() => {
    if (!hasAutoStarted.current) {
      hasAutoStarted.current = true;
      startProxyChain();
    }
  }, []);

  return (
    <div className="toolbar">
      <div className="toolbar-title">Proximus</div>
      <div className="toolbar-statuses">
        {statuses.map((s) => (
          <StatusBadge key={s.name} {...s} />
        ))}
      </div>
      {status && <div className="toolbar-status">{status}</div>}
      {error && <div className="toolbar-error">{error}</div>}
      <div className="toolbar-actions">
        <button
          className="btn btn-start"
          onClick={handleRestart}
          disabled={starting}
        >
          {starting ? "Starting..." : "Restart"}
        </button>
      </div>
    </div>
  );
};
