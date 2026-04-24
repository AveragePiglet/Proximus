import React, { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { StatusBadge } from "./StatusBadge";
import { useProcessStatus } from "../hooks/useProcessStatus";
import DependencyDialog from "./DependencyDialog";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Poll until copilot-proxy reports running, or timeout */
const waitForCopilotProxy = async (
  _port: number,
  timeoutMs = 15000
): Promise<void> => {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const statuses = await invoke<Array<{ name: string; running: boolean }>>(
        "get_process_statuses"
      );
      const copilot = statuses.find((s) => s.name === "copilot-proxy");
      if (copilot?.running) return;
    } catch {}
    await sleep(500);
  }
  // Timeout — proceed anyway, model-rewriter will give a clear error if upstream isn't ready
};

interface DepStatus {
  claude_installed: boolean;
  copilot_api_installed: boolean;
  copilot_cli_installed: boolean;
}

export const Toolbar: React.FC = () => {
  const statuses = useProcessStatus();
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [depCheck, setDepCheck] = useState<DepStatus | null>(null);
  const hasAutoStarted = useRef(false);
  // Resolve function stored so DependencyDialog can unblock startup
  const depResolveRef = useRef<(() => void) | null>(null);

  const checkAndInstallDeps = useCallback(async (): Promise<boolean> => {
    setStatus("Checking dependencies...");
    try {
      const deps = await invoke<DepStatus>("check_dependencies");
      if (deps.claude_installed && deps.copilot_api_installed && deps.copilot_cli_installed) {
        return true; // all good
      }
      // Show dialog and wait for resolution
      setDepCheck(deps);
      return new Promise<boolean>((resolve) => {
        depResolveRef.current = () => resolve(true);
      });
    } catch {
      return true; // if check itself fails, proceed anyway
    }
  }, []);

  const handleDepResolved = useCallback(() => {
    setDepCheck(null);
    depResolveRef.current?.();
    depResolveRef.current = null;
  }, []);

  const handleDepSkip = useCallback(() => {
    setDepCheck(null);
    depResolveRef.current?.();
    depResolveRef.current = null;
  }, []);

  const startProxyChain = async () => {
    setError(null);
    setStarting(true);

    // Check dependencies first
    const depsOk = await checkAndInstallDeps();
    if (!depsOk) {
      setStarting(false);
      setStatus(null);
      return;
    }

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

    setStatus(`Waiting for copilot proxy on :${proxyPort}...`);
    await waitForCopilotProxy(proxyPort);

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

  // Auto-start on mount — skip proxy chain if cli_mode is "copilot"
  useEffect(() => {
    if (!hasAutoStarted.current) {
      hasAutoStarted.current = true;
      invoke<{ cli_mode: string }>("get_app_settings")
        .then((s) => {
          if (s.cli_mode !== "copilot") {
            startProxyChain();
          }
        })
        .catch(() => startProxyChain()); // if settings fail, start anyway
    }
  }, []);

  return (
    <>
      <div className="toolbar">
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
      {depCheck && (
        <DependencyDialog
          claudeMissing={!depCheck.claude_installed}
          copilotApiMissing={!depCheck.copilot_api_installed}
          copilotCliMissing={!depCheck.copilot_cli_installed}
          onResolved={handleDepResolved}
          onSkip={handleDepSkip}
        />
      )}
    </>
  );
};
