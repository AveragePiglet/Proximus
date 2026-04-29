import React, { useState, useEffect, useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import DependencyDialog from "./DependencyDialog";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const waitForCopilotProxy = async (_port: number, timeoutMs = 15000): Promise<void> => {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const statuses = await invoke<Array<{ name: string; running: boolean }>>("get_process_statuses");
      const copilot = statuses.find((s) => s.name === "copilot-proxy");
      if (copilot?.running) return;
    } catch {}
    await sleep(500);
  }
};

interface DepStatus {
  claude_installed: boolean;
  copilot_api_installed: boolean;
  copilot_cli_installed: boolean;
}

export const ChromeBar: React.FC = () => {
  // ── Window controls ──
  const [maximized, setMaximized] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    appWindow.isMaximized().then(setMaximized);
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const handleMinimize  = useCallback(() => appWindow.minimize(), []);
  const handleMaximize  = useCallback(() => appWindow.toggleMaximize(), []);
  const handleClose     = useCallback(() => appWindow.close(), []);

  // ── Proxy controls (from Toolbar) ──
  const [error, setError]     = useState<string | null>(null);
  const [status, setStatus]   = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [depCheck, setDepCheck] = useState<DepStatus | null>(null);
  const hasAutoStarted = useRef(false);
  const depResolveRef  = useRef<(() => void) | null>(null);

  const checkAndInstallDeps = useCallback(async (): Promise<boolean> => {
    setStatus("Checking…");
    try {
      const deps = await invoke<DepStatus>("check_dependencies");
      if (deps.claude_installed && deps.copilot_api_installed && deps.copilot_cli_installed) return true;
      setDepCheck(deps);
      return new Promise<boolean>((resolve) => { depResolveRef.current = () => resolve(true); });
    } catch { return true; }
  }, []);

  const handleDepResolved = useCallback(() => { setDepCheck(null); depResolveRef.current?.(); depResolveRef.current = null; }, []);
  const handleDepSkip     = useCallback(() => { setDepCheck(null); depResolveRef.current?.(); depResolveRef.current = null; }, []);

  const spawnActiveTabPtys = useCallback(async () => {
    setStatus("Spawning…");
    try {
      const tabs = await invoke<Array<{ id: string; status: string }>>("get_tabs");
      for (const tab of tabs) {
        if (tab.status === "active") {
          await invoke("spawn_tab_pty", { tabId: tab.id }).catch((e: unknown) =>
            console.warn(`Failed to spawn PTY for tab ${tab.id}:`, e)
          );
        }
      }
    } catch (e) { console.warn("Failed to spawn tab PTYs:", e); }
  }, []);

  const startProxyChain = async () => {
    setError(null);
    setStarting(true);
    const depsOk = await checkAndInstallDeps();
    if (!depsOk) { setStarting(false); setStatus(null); return; }

    let proxyPort: number;
    try {
      setStatus("Starting proxy…");
      proxyPort = await invoke<number>("start_copilot_proxy");
    } catch (e) { setError(`Proxy failed: ${e}`); setStarting(false); setStatus(null); return; }

    setStatus(`Waiting :${proxyPort}…`);
    await waitForCopilotProxy(proxyPort);

    try {
      setStatus("Starting rewriter…");
      await invoke<number>("start_model_rewriter", { upstreamPort: proxyPort });
    } catch (e) { setError(`Rewriter failed: ${e}`); setStarting(false); setStatus(null); return; }

    await spawnActiveTabPtys();
    setStatus(null);
    setStarting(false);
  };

  const startForCurrentMode = useCallback(async () => {
    setError(null);
    setStarting(true);
    try {
      const settings = await invoke<{ cli_mode: string }>("get_app_settings");
      if (settings.cli_mode === "copilot") {
        await spawnActiveTabPtys();
      } else {
        await startProxyChain();
        return;
      }
    } catch { await startProxyChain(); return; }
    finally { setStatus(null); setStarting(false); }
  }, [spawnActiveTabPtys]);

  const handleRestart = async () => {
    setError(null);
    setStatus("Restarting…");
    try { await invoke("stop_services"); } catch (_) {}
    await sleep(1000);
    await startForCurrentMode();
  };

  useEffect(() => {
    if (!hasAutoStarted.current) {
      hasAutoStarted.current = true;
      startForCurrentMode();
    }
  }, [startForCurrentMode]);

  return (
    <>
      <div className="chrome-bar" data-tauri-drag-region>
        {/* Left: wordmark */}
        <div className="chrome-bar-left" data-tauri-drag-region>
          <span className="chrome-bar-icon" data-tauri-drag-region>⚡</span>
          <span className="chrome-bar-wordmark" data-tauri-drag-region>Proximus</span>
        </div>

        {/* Centre: status/error text only */}
        <div className="chrome-bar-centre" data-tauri-drag-region>
          {status && <span className="chrome-bar-status">{status}</span>}
          {error  && <span className="chrome-bar-error" title={error}>{error}</span>}
        </div>

        {/* Right: restart + window controls */}
        <div className="chrome-bar-right">
          <button
            className="chrome-restart-btn"
            onClick={handleRestart}
            disabled={starting}
            title="Restart proxy chain"
          >
            {starting ? "…" : "↺"}
          </button>
          <div className="chrome-divider" />
          <button className="titlebar-btn titlebar-minimize" onClick={handleMinimize} title="Minimize">
            <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
          </button>
          <button className="titlebar-btn titlebar-maximize" onClick={handleMaximize} title={maximized ? "Restore" : "Maximize"}>
            {maximized ? (
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
                <rect x="2" y="0" width="8" height="8" rx="1" />
                <rect x="0" y="2" width="8" height="8" rx="1" fill="var(--bg-secondary)" />
                <rect x="0" y="2" width="8" height="8" rx="1" />
              </svg>
            ) : (
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
                <rect x="0.5" y="0.5" width="9" height="9" rx="1" />
              </svg>
            )}
          </button>
          <button className="titlebar-btn titlebar-close" onClick={handleClose} title="Close">
            <svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" strokeWidth="1.2">
              <line x1="0" y1="0" x2="10" y2="10" />
              <line x1="10" y1="0" x2="0" y2="10" />
            </svg>
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
